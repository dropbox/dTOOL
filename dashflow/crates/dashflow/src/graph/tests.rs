// Import everything from parent module (graph/mod.rs)
use super::*;
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{BoxedNode, Node};
    use crate::state::AgentState;

    #[test]
    fn test_graph_builder() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", END);
        graph.set_entry_point("node1");

        let compiled = graph.compile();
        assert!(compiled.is_ok());
    }

    #[test]
    fn test_graph_validation_no_entry_point() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

        let result = graph.compile();
        assert!(matches!(result, Err(Error::NoEntryPoint)));
    }

    #[test]
    fn test_graph_validation_missing_node() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("node1", "node2"); // node2 doesn't exist
        graph.set_entry_point("node1");

        let result = graph.compile();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_conditional_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("next".to_string(), "node2".to_string());
        routes.insert("end".to_string(), END.to_string());

        graph.add_conditional_edges("node1", |_state: &AgentState| "next".to_string(), routes);
        graph.set_entry_point("node1");

        let compiled = graph.compile();
        assert!(compiled.is_ok());
    }

    #[test]
    fn test_validate_unreachable_nodes() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", END);
        graph.set_entry_point("node1");
        // node "unreachable" has no incoming edges

        let warnings = graph.validate();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unreachable"));
    }

    #[test]
    fn test_validate_cycles() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));

        // Create a cycle: node1 -> node2 -> node1
        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node1");
        graph.set_entry_point("node1");

        let warnings = graph.validate();
        assert!(warnings.iter().any(|w| w.contains("cycles")));
    }

    #[test]
    fn test_validate_no_warnings() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", END);
        graph.set_entry_point("node1");

        let warnings = graph.validate();
        assert_eq!(warnings.len(), 0);
    }

    // ===== Default Validation Tests (Opt-Out Pattern) =====

    #[test]
    fn test_compile_default_validation_rejects_unreachable() {
        // Tests that compile() rejects graphs with unreachable nodes by default
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("entry", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("entry", END);
        graph.set_entry_point("entry");

        // Default compile() should fail due to unreachable node
        let result = graph.compile();
        assert!(result.is_err());
        match result {
            Err(e) => {
                assert!(e.to_string().contains("unreachable"));
                assert!(e.to_string().contains("validation"));
            }
            Ok(_) => panic!("Expected compile to fail"),
        }
    }

    #[test]
    fn test_compile_without_validation_allows_unreachable() {
        // Tests that compile_without_validation() allows graphs with unreachable nodes
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("entry", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("entry", END);
        graph.set_entry_point("entry");

        // compile_without_validation() should succeed
        let result = graph.compile_without_validation();
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_default_validation_rejects_empty_routes() {
        // Tests that compile() rejects conditional edges with empty routes by default
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("node2", END);
        graph.set_entry_point("node1");

        // Add conditional edge with empty routes
        let empty_routes: HashMap<String, String> = HashMap::new();
        graph.add_conditional_edges(
            "node1",
            |_: &AgentState| "default".to_string(),
            empty_routes,
        );

        // Default compile() should fail due to empty routes
        let result = graph.compile();
        assert!(result.is_err());
        match result {
            Err(e) => assert!(e.to_string().contains("no routes")),
            Ok(_) => panic!("Expected compile to fail"),
        }
    }

    #[test]
    fn test_compile_without_validation_allows_empty_routes() {
        // Tests that compile_without_validation() allows conditional edges with empty routes
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("node2", END);
        graph.set_entry_point("node1");

        // Add conditional edge with empty routes
        let empty_routes: HashMap<String, String> = HashMap::new();
        graph.add_conditional_edges(
            "node1",
            |_: &AgentState| "default".to_string(),
            empty_routes,
        );

        // compile_without_validation() should succeed
        let result = graph.compile_without_validation();
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_with_merge_default_validation() {
        // Tests that compile_with_merge() also has default validation
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("entry", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("entry", END);
        graph.set_entry_point("entry");

        // Default compile_with_merge() should fail due to unreachable node
        let result = graph.compile_with_merge();
        assert!(result.is_err());
    }

    #[test]
    fn test_compile_with_merge_without_validation() {
        // Tests that compile_with_merge_without_validation() skips advanced validation
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("entry", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("entry", END);
        graph.set_entry_point("entry");

        // compile_with_merge_without_validation() should succeed
        let result = graph.compile_with_merge_without_validation();
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_valid_graph_succeeds() {
        // Tests that compile() succeeds for valid graphs
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("entry", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("process", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("entry", "process");
        graph.add_edge("process", END);
        graph.set_entry_point("entry");

        // All nodes are reachable, no empty routes
        let result = graph.compile();
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_cycles_allowed_with_warning() {
        // Tests that cycles are allowed (with warning) - they don't fail compilation
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node1"); // Creates a cycle
        graph.set_entry_point("node1");

        // Cycles are allowed - should compile successfully
        let result = graph.compile();
        assert!(result.is_ok());
    }

    // ===== StateGraph Builder Tests =====

    #[test]
    fn test_state_graph_new() {
        let graph: StateGraph<AgentState> = StateGraph::new();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
        assert!(graph.conditional_edges.is_empty());
        assert!(graph.parallel_edges.is_empty());
        assert!(graph.entry_point.is_none());
    }

    #[test]
    fn test_state_graph_default() {
        let graph: StateGraph<AgentState> = StateGraph::default();
        assert!(graph.nodes.is_empty());
        assert!(graph.entry_point.is_none());
    }

    #[test]
    fn test_add_node_returns_self() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Test method chaining
        graph
            .add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }))
            .add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }))
            .add_edge("node1", "node2")
            .set_entry_point("node1");

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.entry_point, Some("node1".to_string()));
    }

    #[test]
    fn test_add_edge_returns_self() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n3", |state| Box::pin(async move { Ok(state) }));

        // Test edge chaining
        graph
            .add_edge("n1", "n2")
            .add_edge("n2", "n3")
            .add_edge("n3", END);

        assert_eq!(graph.edges.len(), 3);
    }

    #[test]
    fn test_set_entry_point_returns_self() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

        let result = graph.set_entry_point("node1");
        assert!(result.entry_point.is_some());
    }

    // ===== Edge Type Tests =====

    #[test]
    fn test_add_multiple_simple_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n3", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("n1", "n2");
        graph.add_edge("n1", "n3"); // Multiple edges from same node

        assert_eq!(graph.edges.len(), 2);
    }

    #[test]
    fn test_add_parallel_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker3", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges(
            "start",
            vec![
                "worker1".to_string(),
                "worker2".to_string(),
                "worker3".to_string(),
            ],
        );

        assert_eq!(graph.parallel_edges.len(), 1);
        assert_eq!(graph.parallel_edges[0].to.len(), 3);
    }

    #[test]
    fn test_add_conditional_edges_returns_self() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node3", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("path1".to_string(), "node2".to_string());
        routes.insert("path2".to_string(), "node3".to_string());
        routes.insert("end".to_string(), END.to_string());

        graph.add_conditional_edges("node1", |_: &AgentState| "path1".to_string(), routes);

        assert_eq!(graph.conditional_edges.len(), 1);
        assert_eq!(graph.conditional_edges[0].routes.len(), 3);
    }

    #[test]
    fn test_add_parallel_edges_returns_self() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n3", |state| Box::pin(async move { Ok(state) }));

        let result = graph.add_parallel_edges("n1", vec!["n2".to_string(), "n3".to_string()]);

        assert_eq!(result.parallel_edges.len(), 1);
    }

    #[test]
    fn test_parallel_edges_to_end() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker", |state| Box::pin(async move { Ok(state) }));

        // Parallel edges where one target is END
        graph.add_parallel_edges("start", vec!["worker".to_string(), END.to_string()]);

        graph.set_entry_point("start");
        let result = graph.compile_with_merge();
        assert!(result.is_ok());
    }

    // ===== Validation Tests =====

    #[test]
    fn test_validate_no_entry_point() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

        let warnings = graph.validate();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("No entry point"));
    }

    #[test]
    fn test_validate_conditional_edge_no_routes() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("node1");

        // Add conditional edge with empty routes
        graph.add_conditional_edges("node1", |_: &AgentState| "".to_string(), HashMap::new());

        let warnings = graph.validate();
        assert!(warnings.iter().any(|w| w.contains("no routes defined")));
    }

    #[test]
    fn test_validate_multiple_unreachable_nodes() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("reachable", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable2", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("reachable", END);
        graph.set_entry_point("reachable");

        let warnings = graph.validate();
        assert_eq!(warnings.len(), 2); // Two unreachable nodes
        assert!(warnings.iter().any(|w| w.contains("unreachable1")));
        assert!(warnings.iter().any(|w| w.contains("unreachable2")));
    }

    #[test]
    fn test_validate_complex_cycle() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n3", |state| Box::pin(async move { Ok(state) }));

        // Create a cycle: n1 -> n2 -> n3 -> n1
        graph.add_edge("n1", "n2");
        graph.add_edge("n2", "n3");
        graph.add_edge("n3", "n1");
        graph.set_entry_point("n1");

        let warnings = graph.validate();
        assert!(warnings.iter().any(|w| w.contains("cycles")));
    }

    #[test]
    fn test_validate_conditional_edge_cycle() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("n1", "n2");

        // Conditional edge that can loop back
        let mut routes = HashMap::new();
        routes.insert("loop".to_string(), "n1".to_string());
        routes.insert("end".to_string(), END.to_string());
        graph.add_conditional_edges("n2", |_: &AgentState| "loop".to_string(), routes);

        graph.set_entry_point("n1");

        let warnings = graph.validate();
        assert!(warnings.iter().any(|w| w.contains("cycles")));
    }

    #[test]
    fn test_find_reachable_nodes_via_conditional_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("conditional", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("target", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("start", "conditional");

        let mut routes = HashMap::new();
        routes.insert("go".to_string(), "target".to_string());
        graph.add_conditional_edges("conditional", |_: &AgentState| "go".to_string(), routes);

        graph.set_entry_point("start");

        let warnings = graph.validate();
        // "target" should be reachable via conditional edge
        assert!(!warnings
            .iter()
            .any(|w| w.contains("target") && w.contains("unreachable")));
    }

    #[test]
    fn test_find_reachable_nodes_via_parallel_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("parallel1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("parallel2", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges(
            "start",
            vec!["parallel1".to_string(), "parallel2".to_string()],
        );

        graph.set_entry_point("start");

        let warnings = graph.validate();
        // Both parallel targets should be reachable
        assert!(!warnings
            .iter()
            .any(|w| w.contains("parallel1") && w.contains("unreachable")));
        assert!(!warnings
            .iter()
            .any(|w| w.contains("parallel2") && w.contains("unreachable")));
    }

    // ===== Compile Error Tests =====

    #[test]
    fn test_compile_entry_point_not_in_nodes() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("nonexistent");

        let result = graph.compile_with_merge();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_compile_edge_from_missing_node() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("nonexistent", "node1");
        graph.set_entry_point("node1");

        let result = graph.compile();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_compile_conditional_edge_from_missing_node() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("next".to_string(), "node1".to_string());
        graph.add_conditional_edges("nonexistent", |_: &AgentState| "next".to_string(), routes);

        graph.set_entry_point("node1");

        let result = graph.compile();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_compile_conditional_edge_to_missing_node() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("bad".to_string(), "nonexistent".to_string());
        graph.add_conditional_edges("node1", |_: &AgentState| "bad".to_string(), routes);

        graph.set_entry_point("node1");

        let result = graph.compile();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_compile_parallel_edge_from_missing_node() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_parallel_edges("nonexistent", vec!["node1".to_string()]);
        graph.set_entry_point("node1");

        let result = graph.compile_with_merge();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_compile_parallel_edge_to_missing_node() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_parallel_edges("node1", vec!["nonexistent".to_string()]);
        graph.set_entry_point("node1");

        let result = graph.compile_with_merge();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_compile_success_with_all_edge_types() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("simple", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("conditional", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("parallel1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("parallel2", |state| Box::pin(async move { Ok(state) }));

        // Simple edge
        graph.add_edge("start", "simple");
        graph.add_edge("simple", "conditional");

        // Conditional edge
        let mut routes = HashMap::new();
        routes.insert("path1".to_string(), "parallel1".to_string());
        graph.add_conditional_edges("conditional", |_: &AgentState| "path1".to_string(), routes);

        // Parallel edges
        graph.add_parallel_edges("parallel1", vec!["parallel2".to_string(), END.to_string()]);

        graph.set_entry_point("start");

        let result = graph.compile_with_merge();
        assert!(result.is_ok());
    }

    // ===== Mermaid Diagram Tests =====

    #[test]
    fn test_to_mermaid_empty_graph() {
        let graph: StateGraph<AgentState> = StateGraph::new();
        let diagram = graph.to_mermaid();

        assert!(diagram.starts_with("flowchart TD"));
        assert!(diagram.contains("%% Styling"));
    }

    #[test]
    fn test_to_mermaid_simple_graph() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("node1", "node2");
        graph.add_edge("node2", END);
        graph.set_entry_point("node1");

        let diagram = graph.to_mermaid();

        assert!(diagram.contains("Start([Start]) --> node1"));
        assert!(diagram.contains("node1[node1]"));
        assert!(diagram.contains("node2[node2]"));
        assert!(diagram.contains("node1 --> node2"));
        assert!(diagram.contains("node2 --> End"));
        assert!(diagram.contains("End([End])"));
    }

    #[test]
    fn test_to_mermaid_conditional_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("decision", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("path1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("path2", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("option1".to_string(), "path1".to_string());
        routes.insert("option2".to_string(), "path2".to_string());
        graph.add_conditional_edges("decision", |_: &AgentState| "option1".to_string(), routes);

        graph.set_entry_point("decision");

        let diagram = graph.to_mermaid();

        assert!(diagram.contains("decision -->|option1| path1"));
        assert!(diagram.contains("decision -->|option2| path2"));
    }

    #[test]
    fn test_to_mermaid_parallel_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker2", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges("start", vec!["worker1".to_string(), "worker2".to_string()]);
        graph.set_entry_point("start");

        let diagram = graph.to_mermaid();

        // Parallel edges use ==> notation
        assert!(diagram.contains("start ==> worker1"));
        assert!(diagram.contains("start ==> worker2"));
    }

    #[test]
    fn test_to_mermaid_styling() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("node1");

        let diagram = graph.to_mermaid();

        assert!(diagram.contains("classDef startEnd"));
        assert!(diagram.contains("classDef nodeStyle"));
        assert!(diagram.contains("class Start,End startEnd"));
        assert!(diagram.contains("class node1 nodeStyle"));
    }

    #[test]
    fn test_to_mermaid_no_end_reference() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("node1", "node2");
        // No edges to END
        graph.set_entry_point("node1");

        let diagram = graph.to_mermaid();

        // Should not include END node if not referenced
        assert!(!diagram.contains("End([End])"));
    }

    // ===== Subgraph Tests =====

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    struct ParentState {
        task: String,
        result: String,
    }

    impl crate::state::MergeableState for ParentState {
        fn merge(&mut self, other: &Self) {
            // Last-write-wins for string fields (simpl for tests)
            self.result = other.result.clone();
        }
    }

    // Note: GraphState is auto-implemented via blanket impl in state.rs
    // No need to manually implement it

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    struct ChildState {
        query: String,
        findings: Vec<String>,
    }

    impl crate::state::MergeableState for ChildState {
        fn merge(&mut self, other: &Self) {
            // Append findings from parallel branches
            self.findings.extend(other.findings.clone());
        }
    }

    // Note: GraphState is auto-implemented via blanket impl in state.rs
    // No need to manually implement it

    #[tokio::test]
    async fn test_add_subgraph_with_mapping_success() {
        // Create child graph
        let mut child_graph: StateGraph<ChildState> = StateGraph::new();
        child_graph.add_node_from_fn("search", |mut state: ChildState| {
            Box::pin(async move {
                state.findings.push(format!("Found: {}", state.query));
                Ok(state)
            })
        });
        child_graph.add_edge("search", END);
        child_graph.set_entry_point("search");

        // Create parent graph
        let mut parent_graph: StateGraph<ParentState> = StateGraph::new();

        // Add subgraph with mapping
        let result = parent_graph.add_subgraph_with_mapping(
            "research",
            child_graph,
            |parent: &ParentState| ChildState {
                query: parent.task.clone(),
                findings: Vec::new(),
            },
            |mut parent: ParentState, child: ChildState| {
                parent.result = child.findings.join(", ");
                parent
            },
        );

        assert!(result.is_ok());
        assert!(parent_graph.nodes.contains_key("research"));
    }

    #[tokio::test]
    async fn test_add_subgraph_with_mapping_child_compile_error() {
        // Create invalid child graph (no entry point)
        let child_graph: StateGraph<ChildState> = StateGraph::new();

        // Create parent graph
        let mut parent_graph: StateGraph<ParentState> = StateGraph::new();

        // Try to add subgraph - should fail because child can't compile
        let result = parent_graph.add_subgraph_with_mapping(
            "invalid_subgraph",
            child_graph,
            |parent: &ParentState| ChildState {
                query: parent.task.clone(),
                findings: Vec::new(),
            },
            |parent: ParentState, _child: ChildState| parent,
        );

        assert!(result.is_err());
        assert!(!parent_graph.nodes.contains_key("invalid_subgraph"));
    }

    #[tokio::test]
    async fn test_add_subgraph_execution_through_parent() {
        // Create child graph
        let mut child_graph: StateGraph<ChildState> = StateGraph::new();
        child_graph.add_node_from_fn("process", |mut state: ChildState| {
            Box::pin(async move {
                state.findings.push("processed".to_string());
                Ok(state)
            })
        });
        child_graph.add_edge("process", END);
        child_graph.set_entry_point("process");

        // Create parent graph with subgraph and subsequent node
        let mut parent_graph: StateGraph<ParentState> = StateGraph::new();
        parent_graph
            .add_subgraph_with_mapping(
                "sub",
                child_graph,
                |parent: &ParentState| ChildState {
                    query: parent.task.clone(),
                    findings: Vec::new(),
                },
                |mut parent: ParentState, child: ChildState| {
                    parent.result = child.findings.join(",");
                    parent
                },
            )
            .unwrap();

        parent_graph.add_node_from_fn("verify", |state: ParentState| {
            Box::pin(async move { Ok(state) })
        });

        parent_graph.add_edge("sub", "verify");
        parent_graph.add_edge("verify", END);
        parent_graph.set_entry_point("sub");

        let compiled = parent_graph.compile_with_merge();
        assert!(compiled.is_ok());
    }

    // ===== Deprecated Method Tests =====

    #[test]
    #[allow(deprecated)]
    fn test_add_conditional_edge_deprecated_wrapper() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("next".to_string(), "node2".to_string());
        routes.insert("end".to_string(), END.to_string());

        // Use deprecated method (singular)
        graph.add_conditional_edge("node1", |_: &AgentState| "next".to_string(), routes);

        assert_eq!(graph.conditional_edges.len(), 1);
        assert_eq!(graph.conditional_edges[0].from.as_str(), "node1");
    }

    #[test]
    #[allow(deprecated)]
    fn test_add_parallel_edge_deprecated_wrapper() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker2", |state| Box::pin(async move { Ok(state) }));

        // Use deprecated method (singular)
        graph.add_parallel_edge("start", vec!["worker1".to_string(), "worker2".to_string()]);

        assert_eq!(graph.parallel_edges.len(), 1);
        assert_eq!(graph.parallel_edges[0].from.as_str(), "start");
        assert_eq!(graph.parallel_edges[0].to.len(), 2);
    }

    // ===== Complex Validation Tests =====

    #[test]
    fn test_validate_deeply_nested_cycles() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Create a complex cycle: A -> B -> C -> D -> B
        graph.add_node_from_fn("A", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("B", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("C", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("D", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("A", "B");
        graph.add_edge("B", "C");
        graph.add_edge("C", "D");
        graph.add_edge("D", "B"); // Cycle back to B

        graph.set_entry_point("A");

        let warnings = graph.validate();
        assert!(warnings.iter().any(|w| w.contains("cycles")));
    }

    #[test]
    fn test_validate_multiple_disconnected_components() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Connected component 1
        graph.add_node_from_fn("A", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("B", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("A", "B");
        graph.add_edge("B", END);

        // Disconnected component 2
        graph.add_node_from_fn("X", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("Y", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("X", "Y");

        graph.set_entry_point("A");

        let warnings = graph.validate();
        // Should warn about unreachable nodes X and Y
        assert!(warnings.iter().any(|w| w.contains("unreachable")));
    }

    #[test]
    fn test_validate_parallel_edges_unreachable() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("reachable", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("start", "reachable");
        graph.add_edge("reachable", END);
        // unreachable is never connected
        graph.set_entry_point("start");

        let warnings = graph.validate();
        assert!(warnings.iter().any(|w| w.contains("unreachable")));
    }

    // ===== Builder Pattern Tests =====

    #[test]
    fn test_graph_builder_constructor() {
        let graph: StateGraph<AgentState> = GraphBuilder::builder();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
        assert!(graph.entry_point.is_none());
    }

    #[test]
    fn test_graph_builder_full_chain() {
        let mut graph: GraphBuilder<AgentState> = GraphBuilder::builder();

        graph
            .add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }))
            .add_node_from_fn("middle", |state| Box::pin(async move { Ok(state) }))
            .add_node_from_fn("end_node", |state| Box::pin(async move { Ok(state) }))
            .add_edge("start", "middle")
            .add_edge("middle", "end_node")
            .add_edge("end_node", END)
            .set_entry_point("start");

        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 3);
        assert_eq!(graph.entry_point, Some("start".to_string()));

        let compiled = graph.compile();
        assert!(compiled.is_ok());
    }

    // ===== Edge Case Tests =====

    #[test]
    fn test_add_node_with_duplicate_name() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("duplicate", |mut state| {
            Box::pin(async move {
                state.iteration = 1;
                Ok(state)
            })
        });

        graph.add_node_from_fn("duplicate", |mut state| {
            Box::pin(async move {
                state.iteration = 2;
                Ok(state)
            })
        });

        // Should have only one node (second overwrites first)
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("duplicate"));
    }

    #[test]
    fn test_node_with_very_long_name() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        let long_name = "a".repeat(1000);
        graph.add_node_from_fn(&long_name, |state| Box::pin(async move { Ok(state) }));

        assert!(graph.nodes.contains_key(&long_name));
    }

    #[test]
    fn test_node_with_special_characters() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        let special_names = vec![
            "node-with-dashes",
            "node_with_underscores",
            "node.with.dots",
            "node:with:colons",
            "node/with/slashes",
        ];

        for name in special_names {
            graph.add_node_from_fn(name, |state| Box::pin(async move { Ok(state) }));
        }

        assert_eq!(graph.nodes.len(), 5);
    }

    #[test]
    fn test_compile_graph_with_only_entry_point() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("lonely", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("lonely");
        // No edges at all

        let warnings = graph.validate();
        // Should compile successfully (node can implicitly end)
        let compiled = graph.compile();
        assert!(compiled.is_ok());
        assert!(warnings.is_empty()); // No warnings for valid single-node graph
    }

    #[test]
    fn test_compile_empty_graph() {
        let graph: StateGraph<AgentState> = StateGraph::new();

        // Empty graph with no entry point should fail
        let result = graph.compile();
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("no entry point"));
        }
    }

    #[test]
    fn test_validate_self_loop() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("looper", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("looper", "looper"); // Self-loop
        graph.set_entry_point("looper");

        let warnings = graph.validate();
        assert!(warnings.iter().any(|w| w.contains("cycles")));
    }

    #[test]
    fn test_add_edge_with_empty_string_names() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("", "node2");
        graph.set_entry_point("");

        assert!(graph.nodes.contains_key(""));
        assert_eq!(graph.edges.len(), 1);
    }

    // ===== Additional Edge Case Tests =====

    #[test]
    fn test_add_node_from_fn_with_closure() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        let prefix = "test_".to_string();
        graph.add_node_from_fn("process", move |mut state| {
            let p = prefix.clone();
            Box::pin(async move {
                state.messages.push(format!("{p}message"));
                Ok(state)
            })
        });

        assert!(graph.nodes.contains_key("process"));
        assert_eq!(graph.nodes.len(), 1);
    }

    #[tokio::test]
    async fn test_add_node_from_fn_stateful_closure() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("counter", |mut state| {
            Box::pin(async move {
                state.iteration += 1;
                Ok(state)
            })
        });
        graph.set_entry_point("counter");

        let compiled = graph.compile().expect("compilation should succeed");
        let state = AgentState {
            messages: vec![],
            iteration: 0,
            next: None,
            metadata: serde_json::Value::Null,
        };

        let result = compiled
            .invoke(state)
            .await
            .expect("execution should succeed");
        assert_eq!(result.final_state.iteration, 1);
    }

    #[test]
    fn test_add_node_from_fn_error_handling() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("error_node", |_state| {
            Box::pin(async move { Err(Error::Generic("intentional error".to_string())) })
        });

        assert!(graph.nodes.contains_key("error_node"));
    }

    #[test]
    fn test_add_multiple_conditional_edges_same_source() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("router", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("option_a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("option_b", |state| Box::pin(async move { Ok(state) }));

        let mut routes1 = HashMap::new();
        routes1.insert("a".to_string(), "option_a".to_string());
        routes1.insert("b".to_string(), "option_b".to_string());

        graph.add_conditional_edges("router", |_state: &AgentState| "a".to_string(), routes1);

        // Adding another conditional edge from the same node should work
        let mut routes2 = HashMap::new();
        routes2.insert("x".to_string(), "option_a".to_string());
        routes2.insert("y".to_string(), "option_b".to_string());

        graph.add_conditional_edges("router", |_state: &AgentState| "x".to_string(), routes2);

        assert_eq!(graph.conditional_edges.len(), 2);
    }

    #[test]
    fn test_parallel_edge_ordering_preserved() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n3", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges(
            "start",
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
        );

        assert_eq!(graph.parallel_edges.len(), 1);
        let pe = &graph.parallel_edges[0];
        assert_eq!(pe.to[0], "n1");
        assert_eq!(pe.to[1], "n2");
        assert_eq!(pe.to[2], "n3");
    }

    #[test]
    fn test_conditional_edge_with_empty_routes() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        let routes = HashMap::new();
        graph.add_conditional_edges("start", |_state: &AgentState| "none".to_string(), routes);
        graph.set_entry_point("start");

        let warnings = graph.validate();
        // Empty routes means the conditional edge has no targets defined
        // This should be flagged as a validation warning
        assert!(
            !warnings.is_empty(),
            "Expected warnings for empty conditional routes, but got none"
        );
    }

    #[test]
    fn test_to_mermaid_with_unicode_node_names() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("开始", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("处理", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("开始", "处理");
        graph.add_edge("处理", END);
        graph.set_entry_point("开始");

        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("开始"));
        assert!(mermaid.contains("处理"));
    }

    #[test]
    fn test_to_mermaid_with_special_characters_in_names() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node-with-dashes", |state| {
            Box::pin(async move { Ok(state) })
        });
        graph.add_node_from_fn("node_with_underscores", |state| {
            Box::pin(async move { Ok(state) })
        });
        graph.add_edge("node-with-dashes", "node_with_underscores");
        graph.set_entry_point("node-with-dashes");

        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("node-with-dashes"));
        assert!(mermaid.contains("node_with_underscores"));
    }

    #[test]
    fn test_validate_conditional_edge_to_nonexistent_target() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("a".to_string(), "missing_node".to_string());

        graph.add_conditional_edges("start", |_state: &AgentState| "a".to_string(), routes);
        graph.set_entry_point("start");

        let result = graph.compile_with_merge();
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("missing_node"));
        }
    }

    #[test]
    fn test_validate_parallel_edge_to_nonexistent_target() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_parallel_edges("start", vec!["missing_node".to_string()]);
        graph.set_entry_point("start");

        let result = graph.compile_with_merge();
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("missing_node"));
        }
    }

    #[test]
    fn test_graph_clone_behavior() {
        let mut graph1: StateGraph<AgentState> = StateGraph::new();

        graph1.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph1.add_edge("node1", END);
        graph1.set_entry_point("node1");

        // StateGraph doesn't implement Clone by design (contains boxed trait objects)
        // Verify that the graph maintains state correctly
        assert_eq!(graph1.nodes.len(), 1);
        assert_eq!(graph1.edges.len(), 1);
        assert_eq!(graph1.entry_point, Some("node1".to_string()));
    }

    #[test]
    fn test_multiple_edges_to_end() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node3", |state| Box::pin(async move { Ok(state) }));

        // All nodes can independently terminate
        graph.add_edge("node1", END);
        graph.add_edge("node2", END);
        graph.add_edge("node3", END);

        graph.set_entry_point("node1");

        let warnings = graph.validate();
        // node2 and node3 should be unreachable
        assert!(warnings.iter().any(|w| w.contains("unreachable")));
    }

    #[test]
    fn test_conditional_edge_with_all_routes_to_end() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("router", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("terminate".to_string(), END.to_string());
        routes.insert("also_terminate".to_string(), END.to_string());

        graph.add_conditional_edges(
            "router",
            |_state: &AgentState| "terminate".to_string(),
            routes,
        );

        graph.set_entry_point("router");

        let warnings = graph.validate();
        // Should validate successfully - all routes lead to END
        assert!(
            !warnings.iter().any(|w| w.contains("error")),
            "Unexpected error: {:?}",
            warnings
        );
    }

    #[test]
    fn test_find_reachable_nodes_complex_graph() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Create a complex graph with multiple paths
        graph.add_node_from_fn("entry", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("branch_a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("branch_b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("merge", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("a".to_string(), "branch_a".to_string());
        routes.insert("b".to_string(), "branch_b".to_string());

        graph.add_conditional_edges("entry", |_state: &AgentState| "a".to_string(), routes);
        graph.add_edge("branch_a", "merge");
        graph.add_edge("branch_b", "merge");
        graph.add_edge("merge", END);

        graph.set_entry_point("entry");

        let warnings = graph.validate();
        // unreachable should be flagged
        assert!(warnings.iter().any(|w| w.contains("unreachable")));
    }

    #[test]
    fn test_has_cycles_with_conditional_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node_a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node_b", |state| Box::pin(async move { Ok(state) }));

        // Create a cycle through conditional edges
        let mut routes = HashMap::new();
        routes.insert("loop".to_string(), "node_b".to_string());

        graph.add_conditional_edges("node_a", |_state: &AgentState| "loop".to_string(), routes);
        graph.add_edge("node_b", "node_a");

        graph.set_entry_point("node_a");

        let warnings = graph.validate();
        assert!(warnings.iter().any(|w| w.contains("cycles")));
    }

    #[test]
    fn test_add_node_overwrites_existing() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node", |mut state| {
            Box::pin(async move {
                state.iteration = 1;
                Ok(state)
            })
        });

        // Add the same node name again - should overwrite
        graph.add_node_from_fn("node", |mut state| {
            Box::pin(async move {
                state.iteration = 2;
                Ok(state)
            })
        });

        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("node"));
    }

    // ===== Direct add_node with Node trait Tests =====

    use async_trait::async_trait;

    struct CustomNode;

    #[async_trait]
    impl Node<AgentState> for CustomNode {
        async fn execute(&self, state: AgentState) -> Result<AgentState> {
            let mut new_state = state;
            new_state.messages.push("custom".to_string());
            Ok(new_state)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_add_node_with_custom_node_trait() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Test direct add_node with Node trait implementation
        graph.add_node("custom", CustomNode);

        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("custom"));
    }

    #[tokio::test]
    async fn test_add_node_custom_node_execution() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node("custom", CustomNode);
        graph.add_edge("custom", END);
        graph.set_entry_point("custom");

        let compiled = graph.compile().expect("compilation should succeed");
        let state = AgentState {
            messages: vec![],
            iteration: 0,
            next: None,
            metadata: serde_json::Value::Null,
        };

        let result = compiled
            .invoke(state)
            .await
            .expect("execution should succeed");
        assert_eq!(result.final_state.messages, vec!["custom".to_string()]);
    }

    #[test]
    fn test_add_node_chaining_with_node_trait() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph
            .add_node("custom1", CustomNode)
            .add_node("custom2", CustomNode)
            .add_edge("custom1", "custom2")
            .set_entry_point("custom1");

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
    }

    struct ErrorNode;

    #[async_trait]
    impl Node<AgentState> for ErrorNode {
        async fn execute(&self, _state: AgentState) -> Result<AgentState> {
            Err(Error::Generic("node error".to_string()))
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_add_node_error_node_trait() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node("error", ErrorNode);
        graph.set_entry_point("error");

        let compiled = graph.compile().expect("compilation should succeed");
        let state = AgentState {
            messages: vec![],
            iteration: 0,
            next: None,
            metadata: serde_json::Value::Null,
        };

        let result = compiled.invoke(state).await;
        assert!(result.is_err());
    }

    // ===== Additional Subgraph Edge Cases =====

    #[tokio::test]
    async fn test_subgraph_with_sequential_nodes() {
        // Changed from parallel to sequential to work with add_subgraph_with_mapping
        let mut child_graph: StateGraph<ChildState> = StateGraph::new();
        child_graph.add_node_from_fn("worker1", |mut state| {
            Box::pin(async move {
                state.findings.push("w1".to_string());
                Ok(state)
            })
        });
        child_graph.add_node_from_fn("worker2", |mut state| {
            Box::pin(async move {
                state.findings.push("w2".to_string());
                Ok(state)
            })
        });
        child_graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        // Use sequential edges instead of parallel
        child_graph.add_edge("start", "worker1");
        child_graph.add_edge("worker1", "worker2");
        child_graph.set_entry_point("start");

        let mut parent_graph: StateGraph<ParentState> = StateGraph::new();
        let result = parent_graph.add_subgraph_with_mapping(
            "parallel_sub",
            child_graph,
            |parent: &ParentState| ChildState {
                query: parent.task.clone(),
                findings: Vec::new(),
            },
            |mut parent: ParentState, child: ChildState| {
                parent.result = child.findings.join(",");
                parent
            },
        );

        if let Err(e) = &result {
            eprintln!("Subgraph error: {:?}", e);
        }
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_subgraph_with_parallel_edges() {
        // Test a child subgraph that has parallel edges internally
        // This requires the child to implement MergeableState and compile with compile_with_merge()
        let mut child_graph: StateGraph<ChildState> = StateGraph::new();

        child_graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

        child_graph.add_node_from_fn("worker1", |mut state| {
            Box::pin(async move {
                state.findings.push("w1".to_string());
                Ok(state)
            })
        });

        child_graph.add_node_from_fn("worker2", |mut state| {
            Box::pin(async move {
                state.findings.push("w2".to_string());
                Ok(state)
            })
        });

        child_graph.add_node_from_fn("merge", |state| Box::pin(async move { Ok(state) }));

        // Add parallel edges: start -> worker1 | worker2 -> merge
        child_graph.add_parallel_edges("start", vec!["worker1".to_string(), "worker2".to_string()]);
        child_graph.add_edge("worker1", "merge");
        child_graph.add_edge("worker2", "merge");
        child_graph.add_edge("merge", END);
        child_graph.set_entry_point("start");

        // Compile child with merge support since it has parallel edges
        let compiled_child = child_graph
            .compile_with_merge()
            .expect("Child graph should compile");

        // Create parent graph
        let mut parent_graph: StateGraph<ParentState> = StateGraph::new();

        // Add the compiled child subgraph to parent using SubgraphNode
        use crate::subgraph::SubgraphNode;
        let subgraph_node = SubgraphNode::new(
            "parallel_child",
            compiled_child,
            |parent: &ParentState| ChildState {
                query: parent.task.clone(),
                findings: Vec::new(),
            },
            |mut parent: ParentState, child: ChildState| {
                // Both workers should have executed, merging their findings
                parent.result = child.findings.join(",");
                parent
            },
        );

        parent_graph.add_node("parallel_child", subgraph_node);
        parent_graph.add_edge("parallel_child", END);
        parent_graph.set_entry_point("parallel_child");

        let compiled_parent = parent_graph.compile().expect("Parent graph should compile");

        let initial = ParentState {
            task: "test".to_string(),
            result: String::new(),
        };

        let result = compiled_parent
            .invoke(initial)
            .await
            .expect("Execution should succeed");

        // Both workers should have executed in parallel and findings merged
        let findings = &result.final_state.result;
        assert!(findings.contains("w1"), "Should contain w1");
        assert!(findings.contains("w2"), "Should contain w2");
    }

    #[tokio::test]
    async fn test_subgraph_with_conditional_edges() {
        let mut child_graph: StateGraph<ChildState> = StateGraph::new();
        child_graph.add_node_from_fn("router", |state| Box::pin(async move { Ok(state) }));
        child_graph.add_node_from_fn("path_a", |mut state| {
            Box::pin(async move {
                state.findings.push("a".to_string());
                Ok(state)
            })
        });
        child_graph.add_node_from_fn("path_b", |mut state| {
            Box::pin(async move {
                state.findings.push("b".to_string());
                Ok(state)
            })
        });

        let mut routes = HashMap::new();
        routes.insert("a".to_string(), "path_a".to_string());
        routes.insert("b".to_string(), "path_b".to_string());
        child_graph.add_conditional_edges("router", |_: &ChildState| "a".to_string(), routes);
        child_graph.add_edge("path_a", END);
        child_graph.add_edge("path_b", END);
        child_graph.set_entry_point("router");

        let mut parent_graph: StateGraph<ParentState> = StateGraph::new();
        let result = parent_graph.add_subgraph_with_mapping(
            "conditional_sub",
            child_graph,
            |parent: &ParentState| ChildState {
                query: parent.task.clone(),
                findings: Vec::new(),
            },
            |mut parent: ParentState, child: ChildState| {
                parent.result = child.findings.join(",");
                parent
            },
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_subgraph_duplicate_name_overwrites() {
        let mut child_graph1: StateGraph<ChildState> = StateGraph::new();
        child_graph1.add_node_from_fn("child", |state| Box::pin(async move { Ok(state) }));
        child_graph1.add_edge("child", END);
        child_graph1.set_entry_point("child");

        let mut child_graph2: StateGraph<ChildState> = StateGraph::new();
        child_graph2.add_node_from_fn("child2", |state| Box::pin(async move { Ok(state) }));
        child_graph2.add_edge("child2", END);
        child_graph2.set_entry_point("child2");

        let mut parent_graph: StateGraph<ParentState> = StateGraph::new();

        // Add first subgraph
        parent_graph
            .add_subgraph_with_mapping(
                "sub",
                child_graph1,
                |p: &ParentState| ChildState {
                    query: p.task.clone(),
                    findings: Vec::new(),
                },
                |p: ParentState, _c: ChildState| p,
            )
            .unwrap();

        // Add second subgraph with same name (should overwrite)
        parent_graph
            .add_subgraph_with_mapping(
                "sub",
                child_graph2,
                |p: &ParentState| ChildState {
                    query: p.task.clone(),
                    findings: Vec::new(),
                },
                |p: ParentState, _c: ChildState| p,
            )
            .unwrap();

        assert_eq!(parent_graph.nodes.len(), 1);
        assert!(parent_graph.nodes.contains_key("sub"));
    }

    // ===== Additional Validation Edge Cases =====

    #[test]
    fn test_validate_parallel_edge_with_no_targets() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_parallel_edges("start", vec![]);
        graph.set_entry_point("start");

        let _warnings = graph.validate();
        // Empty parallel edge targets should produce a warning or be handled
        // This tests the edge case of parallel edges with empty target list
        assert!(graph.parallel_edges.len() == 1);
    }

    #[test]
    fn test_compile_with_only_conditional_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("end_node", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("finish".to_string(), "end_node".to_string());
        graph.add_conditional_edges("start", |_: &AgentState| "finish".to_string(), routes);
        graph.add_edge("end_node", END);
        graph.set_entry_point("start");

        let result = graph.compile_with_merge();
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_conditional_edge_routes_to_end() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("finish".to_string(), END.to_string());
        graph.add_conditional_edges("start", |_: &AgentState| "finish".to_string(), routes);
        graph.set_entry_point("start");

        let warnings = graph.validate();
        let result = graph.compile();
        assert!(result.is_ok());
        assert!(!warnings.iter().any(|w| w.contains("error")));
    }

    #[test]
    fn test_to_mermaid_complex_mixed_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("entry", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("router", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("parallel_start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("merge", |state| Box::pin(async move { Ok(state) }));

        // Simple edge
        graph.add_edge("entry", "router");

        // Conditional edge
        let mut routes = HashMap::new();
        routes.insert("parallel".to_string(), "parallel_start".to_string());
        routes.insert("end".to_string(), END.to_string());
        graph.add_conditional_edges("router", |_: &AgentState| "parallel".to_string(), routes);

        // Parallel edges
        graph.add_parallel_edges(
            "parallel_start",
            vec!["worker1".to_string(), "worker2".to_string()],
        );

        graph.add_edge("worker1", "merge");
        graph.add_edge("worker2", "merge");
        graph.add_edge("merge", END);

        graph.set_entry_point("entry");

        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("entry"));
        assert!(mermaid.contains("router"));
        assert!(mermaid.contains("parallel_start"));
        assert!(mermaid.contains("worker1"));
        assert!(mermaid.contains("worker2"));
        assert!(mermaid.contains("merge"));
        assert!(mermaid.contains("==>"));
        assert!(mermaid.contains("-->|"));
    }

    #[test]
    fn test_default_trait_implementation() {
        // Test that StateGraph::default() works
        let graph: StateGraph<AgentState> = Default::default();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
        assert!(graph.conditional_edges.is_empty());
        assert!(graph.parallel_edges.is_empty());
        assert!(graph.entry_point.is_none());
    }

    #[test]
    fn test_add_node_with_string_types() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Test add_node with different string types
        graph.add_node("str_literal", CustomNode);
        graph.add_node(String::from("string_owned"), CustomNode);
        graph.add_node("str_slice".to_string(), CustomNode);

        assert_eq!(graph.nodes.len(), 3);
    }

    #[test]
    fn test_set_entry_point_with_string_types() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));

        // Test with different string types
        graph.set_entry_point("node1");
        assert_eq!(graph.entry_point, Some("node1".to_string()));

        graph.set_entry_point(String::from("node1"));
        assert_eq!(graph.entry_point, Some("node1".to_string()));

        graph.set_entry_point("node1".to_string());
        assert_eq!(graph.entry_point, Some("node1".to_string()));
    }

    #[test]
    fn test_add_edge_with_string_types() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n3", |state| Box::pin(async move { Ok(state) }));

        // Test with different string types
        graph.add_edge("n1", "n2");
        graph.add_edge(String::from("n2"), String::from("n3"));
        graph.add_edge("n3".to_string(), END.to_string());

        assert_eq!(graph.edges.len(), 3);
    }

    #[test]
    fn test_validate_entry_point_has_no_incoming_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("pre_entry", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("entry", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("post_entry", |state| Box::pin(async move { Ok(state) }));

        // Edge pointing to entry point (unusual but not invalid)
        graph.add_edge("pre_entry", "entry");
        graph.add_edge("entry", "post_entry");
        graph.add_edge("post_entry", END);
        graph.set_entry_point("entry");

        // Use compile_without_validation since pre_entry is unreachable from entry point
        // This test verifies that entry point can have incoming edges (unusual but valid)
        let result = graph.compile_without_validation();
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_edge_to_end_from_nonexistent_node() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("nonexistent", END);
        graph.set_entry_point("node1");

        let result = graph.compile();
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("nonexistent"));
        }
    }

    #[test]
    fn test_parallel_edges_with_single_target() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("single", |state| Box::pin(async move { Ok(state) }));

        // Parallel edge with just one target (unusual but valid)
        graph.add_parallel_edges("start", vec!["single".to_string()]);
        graph.add_edge("single", END);
        graph.set_entry_point("start");

        let result = graph.compile_with_merge();
        assert!(result.is_ok());
    }

    #[test]
    fn test_conditional_edges_single_route() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("router", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("target", |state| Box::pin(async move { Ok(state) }));

        // Conditional edge with just one route (unusual but valid)
        let mut routes = HashMap::new();
        routes.insert("only".to_string(), "target".to_string());
        graph.add_conditional_edges("router", |_: &AgentState| "only".to_string(), routes);
        graph.add_edge("target", END);
        graph.set_entry_point("router");

        let result = graph.compile();
        assert!(result.is_ok());
    }

    #[test]
    fn test_to_mermaid_entry_point_not_set() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("node1", "node2");

        // No entry point set
        let mermaid = graph.to_mermaid();

        // Should not include Start node if entry point not set
        assert!(!mermaid.contains("Start([Start])"));
    }

    #[test]
    fn test_validate_all_nodes_unreachable() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node3", |state| Box::pin(async move { Ok(state) }));

        // No edges, no entry point
        let warnings = graph.validate();

        // Should warn about no entry point
        assert!(warnings.iter().any(|w| w.contains("No entry point")));
    }

    #[test]
    fn test_multiple_parallel_edges_from_same_node() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("a1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("a2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b2", |state| Box::pin(async move { Ok(state) }));

        // Add multiple parallel edge groups from same source
        graph.add_parallel_edges("start", vec!["a1".to_string(), "a2".to_string()]);
        graph.add_parallel_edges("start", vec!["b1".to_string(), "b2".to_string()]);

        assert_eq!(graph.parallel_edges.len(), 2);
    }

    // ===== Comprehensive Edge Case Tests =====

    #[test]
    fn test_self_loop_simple_edge() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("node", "node"); // Self-loop
        graph.set_entry_point("node");

        let warnings = graph.validate();
        // Self-loop should trigger cycle warning
        assert!(warnings.iter().any(|w| w.contains("cycle")));
    }

    #[test]
    fn test_self_loop_conditional_edge() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("loop".to_string(), "node".to_string());
        routes.insert("end".to_string(), END.to_string());

        graph.add_conditional_edges("node", |_: &AgentState| "loop".to_string(), routes);
        graph.set_entry_point("node");

        let warnings = graph.validate();
        // Self-loop in conditional edge should trigger cycle warning
        assert!(warnings.iter().any(|w| w.contains("cycle")));
    }

    #[test]
    fn test_self_loop_parallel_edge() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node", |state| Box::pin(async move { Ok(state) }));
        // Parallel edge that includes itself
        graph.add_parallel_edges("node", vec!["node".to_string()]);
        graph.set_entry_point("node");

        let warnings = graph.validate();
        // Self-loop in parallel edge should trigger cycle warning
        assert!(warnings.iter().any(|w| w.contains("cycle")));
    }

    #[test]
    fn test_cycle_through_parallel_edges_multi_path() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("c", |state| Box::pin(async move { Ok(state) }));

        // a -> [b, c] in parallel, then both cycle back
        graph.add_parallel_edges("a", vec!["b".to_string(), "c".to_string()]);
        graph.add_edge("b", "a"); // Cycle back
        graph.add_edge("c", "a"); // Cycle back
        graph.set_entry_point("a");

        let warnings = graph.validate();
        assert!(warnings.iter().any(|w| w.contains("cycle")));
    }

    #[test]
    fn test_mixed_edge_types_cycle() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("c", |state| Box::pin(async move { Ok(state) }));

        // Simple edge a -> b
        graph.add_edge("a", "b");

        // Conditional edge b -> c
        let mut routes = HashMap::new();
        routes.insert("go".to_string(), "c".to_string());
        graph.add_conditional_edges("b", |_: &AgentState| "go".to_string(), routes);

        // Parallel edge c -> [a] (cycle back)
        graph.add_parallel_edges("c", vec!["a".to_string()]);
        graph.set_entry_point("a");

        let warnings = graph.validate();
        assert!(warnings.iter().any(|w| w.contains("cycle")));
    }

    #[test]
    fn test_empty_graph_to_mermaid() {
        let graph: StateGraph<AgentState> = StateGraph::new();
        let mermaid = graph.to_mermaid();

        // Should produce valid (though minimal) mermaid
        assert!(mermaid.contains("flowchart TD"));
        // No entry point means no "Start([Start])" node
        assert!(!mermaid.contains("Start([Start])"));
    }

    #[test]
    fn test_graph_with_only_nodes_no_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n3", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("n1");

        let warnings = graph.validate();
        // n2 and n3 should be unreachable
        assert!(
            warnings
                .iter()
                .filter(|w| w.contains("unreachable"))
                .count()
                >= 2
        );
    }

    #[test]
    fn test_find_reachable_nodes_empty_graph() {
        let graph: StateGraph<AgentState> = StateGraph::new();
        let reachable = graph.find_reachable_nodes();
        assert!(reachable.is_empty());
    }

    #[test]
    fn test_find_reachable_nodes_single_node() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("only", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("only");

        let reachable = graph.find_reachable_nodes();
        assert_eq!(reachable.len(), 1);
        assert!(reachable.contains("only"));
    }

    #[test]
    fn test_has_cycles_empty_graph() {
        let graph: StateGraph<AgentState> = StateGraph::new();
        assert!(!graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_single_node_no_self_loop() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("node", |state| Box::pin(async move { Ok(state) }));
        assert!(!graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_linear_chain() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("c", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("a", "b");
        graph.add_edge("b", "c");
        graph.add_edge("c", END);

        assert!(!graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_two_node_cycle() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("a", "b");
        graph.add_edge("b", "a"); // Cycle

        assert!(graph.has_cycles());
    }

    // ===== Additional Edge Case Tests for Improved Coverage =====

    #[test]
    fn test_has_cycles_self_loop() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("loop", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("loop", "loop"); // Self-loop creates cycle
        assert!(graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_with_conditional_edges_back_loop() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("c", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("a", "b");

        let mut routes = HashMap::new();
        routes.insert("back".to_string(), "a".to_string()); // Creates cycle
        routes.insert("forward".to_string(), "c".to_string());
        graph.add_conditional_edges("b", |_: &AgentState| "back".to_string(), routes);

        assert!(graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_with_parallel_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("end_node", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges("start", vec!["worker".to_string(), "end_node".to_string()]);
        graph.add_edge("worker", "start"); // Cycle through parallel edge

        assert!(graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_diamond_pattern_no_cycle() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Create diamond pattern (no cycle)
        // Diamond: A -> B -> D
        //          A -> C -> D
        // This is a DAG with convergent paths. D is reachable through two paths,
        // but there is no cycle (no path from D back to A, B, or C).
        // Previous BFS implementation gave false positive for this pattern.
        // Fixed with DFS + recursion stack approach.
        graph.add_node_from_fn("a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("c", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("d", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("a", "b");
        graph.add_edge("a", "c");
        graph.add_edge("b", "d");
        graph.add_edge("c", "d");
        graph.add_edge("d", END);

        assert!(!graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_diamond_with_parallel_edges_no_cycle() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Create diamond pattern with parallel edges (still no cycle)
        // Diamond: A -parallel-> [B, C] -> D
        graph.add_node_from_fn("a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("c", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("d", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges("a", vec!["b".to_string(), "c".to_string()]);
        graph.add_edge("b", "d");
        graph.add_edge("c", "d");
        graph.add_edge("d", END);

        assert!(!graph.has_cycles());
    }

    #[test]
    fn test_find_reachable_nodes_with_all_edge_types() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("simple", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("conditional1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("conditional2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("parallel1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("parallel2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("start");

        // Simple edge
        graph.add_edge("start", "simple");

        // Conditional edges
        let mut routes = HashMap::new();
        routes.insert("path1".to_string(), "conditional1".to_string());
        routes.insert("path2".to_string(), "conditional2".to_string());
        graph.add_conditional_edges("simple", |_: &AgentState| "path1".to_string(), routes);

        // Parallel edges
        graph.add_parallel_edges(
            "conditional1",
            vec!["parallel1".to_string(), "parallel2".to_string()],
        );

        let reachable = graph.find_reachable_nodes();

        assert_eq!(reachable.len(), 6); // All except "unreachable"
        assert!(reachable.contains("start"));
        assert!(reachable.contains("simple"));
        assert!(reachable.contains("conditional1"));
        assert!(reachable.contains("conditional2"));
        assert!(reachable.contains("parallel1"));
        assert!(reachable.contains("parallel2"));
        assert!(!reachable.contains("unreachable"));
    }

    #[test]
    fn test_find_reachable_nodes_excludes_end() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("middle", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");

        graph.add_edge("start", "middle");
        graph.add_edge("middle", END);

        let reachable = graph.find_reachable_nodes();

        // END is not included in reachable nodes (it's a special sentinel)
        assert_eq!(reachable.len(), 2);
        assert!(reachable.contains("start"));
        assert!(reachable.contains("middle"));
        assert!(!reachable.contains(END));
    }

    #[test]
    fn test_validate_conditional_edge_with_end_route() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("next", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");

        let mut routes = HashMap::new();
        routes.insert("continue".to_string(), "next".to_string());
        routes.insert("finish".to_string(), END.to_string());
        graph.add_conditional_edges("start", |_: &AgentState| "continue".to_string(), routes);

        graph.add_edge("next", END);

        let warnings = graph.validate();
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_validate_parallel_edge_with_end_route() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");

        graph.add_parallel_edges("start", vec!["worker".to_string(), END.to_string()]);
        graph.add_edge("worker", END);

        let warnings = graph.validate();
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_validate_single_conditional_route_to_end() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");

        let mut routes = HashMap::new();
        routes.insert("end".to_string(), END.to_string());
        graph.add_conditional_edges("start", |_: &AgentState| "end".to_string(), routes);

        let warnings = graph.validate();
        // Should have no warnings - it's valid to have conditional edge directly to END
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_validate_multiple_entry_points_override() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("first", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("second", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("first");
        graph.set_entry_point("second"); // Override

        graph.add_edge("second", END);

        let warnings = graph.validate();
        // "first" should be unreachable now
        assert!(warnings
            .iter()
            .any(|w| w.contains("first") && w.contains("unreachable")));
    }

    #[test]
    fn test_add_node_duplicate_name_overwrites() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("duplicate", |state| {
            Box::pin(async move {
                let mut s = state;
                s.messages.push("first".into());
                Ok(s)
            })
        });

        // Add same name again - should overwrite
        graph.add_node_from_fn("duplicate", |state| {
            Box::pin(async move {
                let mut s = state;
                s.messages.push("second".into());
                Ok(s)
            })
        });

        assert_eq!(graph.nodes.len(), 1);
    }

    #[test]
    fn test_add_edge_same_source_multiple_times() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("hub", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("spoke1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("spoke2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("spoke3", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("hub", "spoke1");
        graph.add_edge("hub", "spoke2");
        graph.add_edge("hub", "spoke3");

        // Multiple edges from same source should all be added
        assert_eq!(graph.edges.len(), 3);
    }

    #[test]
    fn test_add_edge_to_end_multiple_nodes() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("final1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("final2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("final3", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("final1", END);
        graph.add_edge("final2", END);
        graph.add_edge("final3", END);

        assert_eq!(graph.edges.len(), 3);
        assert!(graph.edges.iter().all(|e| e.to.as_str() == END));
    }

    #[test]
    fn test_conditional_edges_empty_condition_string() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("target", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");

        let mut routes = HashMap::new();
        routes.insert("".to_string(), "target".to_string()); // Empty string as condition
        routes.insert("normal".to_string(), END.to_string());
        graph.add_conditional_edges("start", |_: &AgentState| "".to_string(), routes);

        graph.add_edge("target", END);

        let result = graph.compile_with_merge();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parallel_edges_single_target() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("target", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");

        // Parallel edges with only one target (technically valid, though unusual)
        graph.add_parallel_edges("start", vec!["target".to_string()]);
        graph.add_edge("target", END);

        let result = graph.compile_with_merge();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parallel_edges_empty_targets() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");

        // Parallel edges with no targets
        graph.add_parallel_edges("start", vec![]);

        // The validate() function doesn't specifically warn about empty parallel edges
        // It's technically valid (though unusual) - the node just has no outgoing edges
        // The graph is still reachable and has no cycles, so no warnings are generated
        assert_eq!(graph.parallel_edges.len(), 1);
        assert_eq!(graph.parallel_edges[0].to.len(), 0);
    }

    #[test]
    fn test_node_name_with_spaces() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node with spaces", |state| {
            Box::pin(async move { Ok(state) })
        });
        graph.add_node_from_fn("another node", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("node with spaces");

        graph.add_edge("node with spaces", "another node");
        graph.add_edge("another node", END);

        let result = graph.compile_with_merge();
        assert!(result.is_ok());
    }

    #[test]
    fn test_node_name_with_special_chars() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node-with-dashes", |state| {
            Box::pin(async move { Ok(state) })
        });
        graph.add_node_from_fn("node_with_underscores", |state| {
            Box::pin(async move { Ok(state) })
        });
        graph.add_node_from_fn("node.with.dots", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("node-with-dashes");

        graph.add_edge("node-with-dashes", "node_with_underscores");
        graph.add_edge("node_with_underscores", "node.with.dots");
        graph.add_edge("node.with.dots", END);

        let result = graph.compile();
        assert!(result.is_ok());
    }

    #[test]
    fn test_very_long_node_name() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        let long_name = "a".repeat(1000); // 1000 character node name
        graph.add_node_from_fn(&long_name, |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point(&long_name);
        graph.add_edge(&long_name, END);

        let result = graph.compile();
        assert!(result.is_ok());
    }

    #[test]
    fn test_conditional_edges_many_routes() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("router", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("router");

        let mut routes = HashMap::new();
        // Add 100 different routes
        for i in 0..100 {
            let node_name = format!("target_{}", i);
            graph.add_node_from_fn(&node_name, |state| Box::pin(async move { Ok(state) }));
            routes.insert(format!("route_{}", i), node_name.clone());
            graph.add_edge(&node_name, END);
        }

        graph.add_conditional_edges("router", |_: &AgentState| "route_0".to_string(), routes);

        assert_eq!(graph.nodes.len(), 101); // router + 100 targets (check before compile)

        let result = graph.compile();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parallel_edges_many_targets() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("fanout", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("fanout");

        let mut targets = Vec::new();
        // Add 50 parallel targets
        for i in 0..50 {
            let node_name = format!("worker_{}", i);
            graph.add_node_from_fn(&node_name, |state| Box::pin(async move { Ok(state) }));
            targets.push(node_name.clone());
            graph.add_edge(&node_name, END);
        }

        graph.add_parallel_edges("fanout", targets);

        assert_eq!(graph.parallel_edges[0].to.len(), 50); // Check before compile

        let result = graph.compile_with_merge();
        assert!(result.is_ok());
    }

    #[test]
    fn test_deeply_nested_linear_graph() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Create a chain of 100 nodes
        for i in 0..100 {
            let node_name = format!("node_{}", i);
            graph.add_node_from_fn(&node_name, |state| Box::pin(async move { Ok(state) }));

            if i == 0 {
                graph.set_entry_point(&node_name);
            } else {
                let prev_name = format!("node_{}", i - 1);
                graph.add_edge(&prev_name, &node_name);
            }
        }

        graph.add_edge("node_99", END);

        let warnings = graph.validate();
        assert_eq!(warnings.len(), 0); // All nodes should be reachable (check before compile)

        let result = graph.compile();
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_validates_conditional_edge_targets() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");

        let mut routes = HashMap::new();
        routes.insert("valid".to_string(), END.to_string());
        routes.insert("invalid".to_string(), "nonexistent".to_string()); // Target doesn't exist
        graph.add_conditional_edges("start", |_: &AgentState| "valid".to_string(), routes);

        let result = graph.compile();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_compile_validates_parallel_edge_targets() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("exists", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");

        // One valid target, one invalid
        graph.add_parallel_edges("start", vec!["exists".to_string(), "missing".to_string()]);

        let result = graph.compile_with_merge();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_builder_alias() {
        let mut graph: StateGraph<AgentState> = StateGraph::builder();

        graph.add_node_from_fn("node", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("node");
        graph.add_edge("node", END);

        let result = graph.compile();
        assert!(result.is_ok());
    }

    #[test]
    fn test_mixed_edge_types_all_to_same_target() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("source1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("source2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("source3", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("target", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("source1");

        // Different edge types all pointing to same target
        graph.add_edge("source1", "target"); // Simple edge

        let mut routes = HashMap::new();
        routes.insert("go".to_string(), "target".to_string());
        graph.add_conditional_edges("source2", |_: &AgentState| "go".to_string(), routes);

        graph.add_parallel_edges("source3", vec!["target".to_string()]);

        graph.add_edge("target", END);

        // Use compile_with_merge_without_validation since source2/source3 are unreachable
        // (this test verifies that multiple incoming edges to the same target work)
        let result = graph.compile_with_merge_without_validation();
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_warns_about_cycles_with_all_edge_types() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("c", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("a");

        // Create cycle using mixed edge types
        graph.add_edge("a", "b"); // Simple

        let mut routes = HashMap::new();
        routes.insert("next".to_string(), "c".to_string());
        graph.add_conditional_edges("b", |_: &AgentState| "next".to_string(), routes); // Conditional

        graph.add_parallel_edges("c", vec!["a".to_string()]); // Parallel back to start

        let warnings = graph.validate();
        assert!(warnings.iter().any(|w| w.contains("cycles")));
    }

    #[test]
    fn test_has_cycles_long_cycle() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Create a cycle through 10 nodes
        for i in 0..10 {
            let node_name = format!("node_{}", i);
            graph.add_node_from_fn(&node_name, |state| Box::pin(async move { Ok(state) }));
        }

        for i in 0..10 {
            let from = format!("node_{}", i);
            let to = format!("node_{}", (i + 1) % 10); // Last connects back to first
            graph.add_edge(&from, &to);
        }

        assert!(graph.has_cycles());
    }

    #[test]
    fn test_find_reachable_nodes_complex_branching() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Create complex tree structure
        graph.add_node_from_fn("root", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("branch1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("branch2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("leaf1a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("leaf1b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("leaf2a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("leaf2b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("isolated", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("root");

        graph.add_parallel_edges("root", vec!["branch1".to_string(), "branch2".to_string()]);
        graph.add_parallel_edges("branch1", vec!["leaf1a".to_string(), "leaf1b".to_string()]);
        graph.add_parallel_edges("branch2", vec!["leaf2a".to_string(), "leaf2b".to_string()]);

        let reachable = graph.find_reachable_nodes();

        assert_eq!(reachable.len(), 7); // All except "isolated"
        assert!(!reachable.contains("isolated"));
        assert!(reachable.contains("root"));
        assert!(reachable.contains("leaf1a"));
        assert!(reachable.contains("leaf2b"));
    }

    // === Edge Case Tests for Coverage Improvement ===

    #[test]
    fn test_validate_conditional_edge_with_empty_routes() {
        // Test the warning for conditional edges with no routes defined
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");

        // Add conditional edge with empty routes map
        let empty_routes = HashMap::new();
        graph.add_conditional_edges(
            "start",
            |_: &AgentState| "nowhere".to_string(),
            empty_routes,
        );

        let warnings = graph.validate();

        // Should warn about empty routes
        assert!(warnings.iter().any(|w| w.contains("no routes defined")));
    }

    #[test]
    fn test_compile_entry_point_references_nonexistent_node() {
        // Test that compile fails when entry point doesn't exist as a node
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("real_node", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("nonexistent_node"); // Entry point doesn't exist

        let result = graph.compile();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_to_mermaid_with_conditional_routes_to_end() {
        // Test mermaid diagram generation with conditional edges to END
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("decision", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("decision");

        let mut routes = HashMap::new();
        routes.insert("finish".to_string(), END.to_string());
        graph.add_conditional_edges("decision", |_: &AgentState| "finish".to_string(), routes);

        let mermaid = graph.to_mermaid();

        // Should include End node
        assert!(mermaid.contains("End([End])"));
        // Should include conditional edge to End
        assert!(mermaid.contains("decision -->|finish| End"));
    }

    #[test]
    fn test_to_mermaid_parallel_edge_to_end() {
        // Test mermaid diagram with parallel edges including END
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");

        graph.add_parallel_edges("start", vec!["node1".to_string(), END.to_string()]);

        let mermaid = graph.to_mermaid();

        // Should include End node
        assert!(mermaid.contains("End([End])"));
        // Should include parallel edge to End (using ==>)
        assert!(mermaid.contains("start ==> End"));
    }

    #[test]
    fn test_to_mermaid_node_styling_with_nodes() {
        // Test that node styling is applied when nodes exist
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("node1");
        graph.add_edge("node1", "node2");
        graph.add_edge("node2", END);

        let mermaid = graph.to_mermaid();

        // Should include node styling
        assert!(mermaid.contains("classDef nodeStyle"));
        // Should apply styling to nodes (order might vary)
        assert!(
            mermaid.contains("class node1,node2 nodeStyle")
                || mermaid.contains("class node2,node1 nodeStyle")
        );
    }

    #[test]
    fn test_to_mermaid_no_node_styling_when_empty() {
        // Test that empty graphs don't apply node styling incorrectly
        let graph: StateGraph<AgentState> = StateGraph::new();
        let mermaid = graph.to_mermaid();

        // Should still define the class but not apply it to empty list
        assert!(mermaid.contains("classDef nodeStyle"));
        // Should have "class  nodeStyle" pattern (empty node list) - but this shouldn't appear
        // Instead, with no nodes, the class line should be omitted or empty
        let lines: Vec<&str> = mermaid.lines().collect();
        let class_application_lines: Vec<&str> = lines
            .iter()
            .filter(|l| l.contains("class ") && l.contains("nodeStyle"))
            .copied()
            .collect();

        // Empty graphs should not have a class application line with empty node list
        if !class_application_lines.is_empty() {
            // If present, should not be "class  nodeStyle" (double space)
            assert!(!class_application_lines
                .iter()
                .any(|l| l.contains("class  nodeStyle")));
        }
    }

    #[test]
    fn test_find_reachable_nodes_with_no_entry_point() {
        // Test that find_reachable_nodes returns empty set when no entry point
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("node1", "node2");

        // No entry point set
        let reachable = graph.find_reachable_nodes();
        assert!(reachable.is_empty());
    }

    #[test]
    fn test_has_cycles_returns_false_for_dag() {
        // Test has_cycles returns false for a proper DAG
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("c", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("a", "b");
        graph.add_edge("b", "c");
        graph.add_edge("c", END);
        graph.set_entry_point("a");

        assert!(!graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_with_self_loop() {
        // Test has_cycles detects self-loops
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("loop_node", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("loop_node", "loop_node"); // Self-loop
        graph.set_entry_point("loop_node");

        assert!(graph.has_cycles());
    }

    #[test]
    fn test_validate_returns_early_without_entry_point() {
        // Test that validate returns early with only entry point warning when no entry point
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("unreachable", |state| Box::pin(async move { Ok(state) }));

        let warnings = graph.validate();

        // Should only have the "No entry point" warning and return early
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("No entry point"));
    }

    #[test]
    fn test_validate_combines_multiple_warnings() {
        // Test that validate can return multiple warnings together
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("loop", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("start");
        graph.add_edge("start", "loop");
        graph.add_edge("loop", "loop"); // Cycle
                                        // "unreachable" has no edges

        // Add conditional edge with empty routes
        let empty_routes = HashMap::new();
        graph.add_conditional_edges("start", |_: &AgentState| "x".to_string(), empty_routes);

        let warnings = graph.validate();

        // Should have multiple warnings:
        // 1. Unreachable node
        // 2. Cycles
        // 3. Empty routes
        assert!(warnings.len() >= 3);
        assert!(warnings.iter().any(|w| w.contains("unreachable")));
        assert!(warnings.iter().any(|w| w.contains("cycles")));
        assert!(warnings.iter().any(|w| w.contains("no routes")));
    }

    #[test]
    fn test_default_implementation() {
        // Test that Default trait creates same state as new()
        let graph1: StateGraph<AgentState> = StateGraph::new();
        let graph2: StateGraph<AgentState> = StateGraph::default();

        // Both should have no nodes, edges, entry point
        assert_eq!(graph1.nodes.len(), graph2.nodes.len());
        assert_eq!(graph1.edges.len(), graph2.edges.len());
        assert_eq!(
            graph1.conditional_edges.len(),
            graph2.conditional_edges.len()
        );
        assert_eq!(graph1.parallel_edges.len(), graph2.parallel_edges.len());
        assert_eq!(graph1.entry_point, graph2.entry_point);
    }

    #[test]
    fn test_add_node_overwrites_duplicate_name() {
        // Test that adding a node with duplicate name overwrites the previous
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node", |mut state| {
            Box::pin(async move {
                state.messages.push("first".to_string());
                Ok(state)
            })
        });

        // Overwrite with different implementation
        graph.add_node_from_fn("node", |mut state| {
            Box::pin(async move {
                state.messages.push("second".to_string());
                Ok(state)
            })
        });

        // Should only have one node
        assert_eq!(graph.nodes.len(), 1);
    }

    #[test]
    fn test_has_cycles_with_conditional_edge_cycle() {
        // Test cycle detection through conditional edges
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("a", "b");

        let mut routes = HashMap::new();
        routes.insert("back".to_string(), "a".to_string()); // Cycle back to a
        graph.add_conditional_edges("b", |_: &AgentState| "back".to_string(), routes);

        graph.set_entry_point("a");

        assert!(graph.has_cycles());
    }

    #[test]
    fn test_find_reachable_nodes_through_mixed_edges() {
        // Test reachability through all edge types in one graph
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("via_simple", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("via_conditional", |state| {
            Box::pin(async move { Ok(state) })
        });
        graph.add_node_from_fn("via_parallel_1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("via_parallel_2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("start");
        graph.add_edge("start", "via_simple");

        let mut routes = HashMap::new();
        routes.insert("go".to_string(), "via_conditional".to_string());
        graph.add_conditional_edges("via_simple", |_: &AgentState| "go".to_string(), routes);

        graph.add_parallel_edges(
            "via_conditional",
            vec!["via_parallel_1".to_string(), "via_parallel_2".to_string()],
        );

        let reachable = graph.find_reachable_nodes();

        assert_eq!(reachable.len(), 5); // All except "unreachable"
        assert!(reachable.contains("start"));
        assert!(reachable.contains("via_simple"));
        assert!(reachable.contains("via_conditional"));
        assert!(reachable.contains("via_parallel_1"));
        assert!(reachable.contains("via_parallel_2"));
        assert!(!reachable.contains("unreachable"));
    }

    #[test]
    fn test_to_mermaid_all_edge_types_combined() {
        // Test mermaid generation with all edge types together
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("simple_target", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("conditional_target", |state| {
            Box::pin(async move { Ok(state) })
        });
        graph.add_node_from_fn("parallel_target", |state| {
            Box::pin(async move { Ok(state) })
        });

        graph.set_entry_point("start");
        graph.add_edge("start", "simple_target");

        let mut routes = HashMap::new();
        routes.insert("cond".to_string(), "conditional_target".to_string());
        graph.add_conditional_edges("simple_target", |_: &AgentState| "cond".to_string(), routes);

        graph.add_parallel_edges("conditional_target", vec!["parallel_target".to_string()]);
        graph.add_edge("parallel_target", END);

        let mermaid = graph.to_mermaid();

        // Should include Start indicator
        assert!(mermaid.contains("Start([Start]) --> start"));
        // Should include simple edge (-->)
        assert!(mermaid.contains("start --> simple_target"));
        // Should include conditional edge with label (-->|label|)
        assert!(mermaid.contains("simple_target -->|cond| conditional_target"));
        // Should include parallel edge (==>)
        assert!(mermaid.contains("conditional_target ==> parallel_target"));
        // Should include END node
        assert!(mermaid.contains("End([End])"));
        assert!(mermaid.contains("parallel_target --> End"));
    }

    #[test]
    fn test_add_node_from_fn_returns_self_for_chaining() {
        // Test that add_node_from_fn returns &mut self for chaining
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph
            .add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }))
            .add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }))
            .set_entry_point("node1")
            .add_edge("node1", "node2")
            .add_edge("node2", END);

        let result = graph.compile_with_merge();
        assert!(result.is_ok());
    }

    #[test]
    fn test_conditional_edge_with_multiple_routes_including_end() {
        // Test conditional edges with multiple routes including END
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("decision", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("continue_node", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("continue".to_string(), "continue_node".to_string());
        routes.insert("finish".to_string(), END.to_string());

        graph.add_conditional_edges("decision", |_: &AgentState| "continue".to_string(), routes);
        graph.set_entry_point("decision");
        graph.add_edge("continue_node", END);

        let result = graph.compile();
        assert!(result.is_ok());
    }

    // ===== Additional Edge Case Tests =====

    #[test]
    fn test_validate_parallel_edges_with_empty_targets() {
        // Test validation when parallel edge has empty target list
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_parallel_edges("node1", vec![]);
        graph.set_entry_point("node1");

        // Graph should compile despite empty parallel edge list
        let result = graph.compile_with_merge();
        assert!(result.is_ok());
    }

    #[test]
    fn test_find_reachable_nodes_isolated_entry_point() {
        // Test find_reachable_nodes with entry point but no edges
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("isolated", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("isolated");

        let reachable = graph.find_reachable_nodes();
        assert_eq!(reachable.len(), 1);
        assert!(reachable.contains("isolated"));
    }

    #[test]
    fn test_has_cycles_single_node_no_edges() {
        // Test cycle detection with single node and no edges
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("lone", |state| Box::pin(async move { Ok(state) }));
        assert!(!graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_self_referential() {
        // Test cycle detection with self-loop
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("loop_node", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("loop_node", "loop_node");

        assert!(graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_diamond_pattern() {
        // Test cycle detection on diamond pattern
        // Diamond: start -> left -> end_node
        //          start -> right -> end_node
        // This is a DAG with convergent paths, not a cycle.
        // Fixed: DFS with recursion stack correctly identifies this as NOT a cycle.
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("left", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("right", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("end_node", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("start", "left");
        graph.add_edge("start", "right");
        graph.add_edge("left", "end_node");
        graph.add_edge("right", "end_node");
        graph.add_edge("end_node", END);

        // Fixed implementation correctly reports no cycle
        assert!(!graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_parallel_edges_create_cycle() {
        // Test cycle detection with parallel edges forming a cycle
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges("n1", vec!["n2".to_string()]);
        graph.add_edge("n2", "n1"); // Create cycle

        assert!(graph.has_cycles());
    }

    #[test]
    fn test_to_mermaid_multiple_entry_points_override() {
        // Test to_mermaid when entry point is changed
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("first", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("second", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("first");
        graph.set_entry_point("second"); // Override

        let diagram = graph.to_mermaid();
        assert!(diagram.contains("Start([Start]) --> second"));
        assert!(!diagram.contains("Start([Start]) --> first"));
    }

    #[test]
    fn test_to_mermaid_nodes_without_edges() {
        // Test to_mermaid with nodes but no edges
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("isolated1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("isolated2", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("isolated1");

        let diagram = graph.to_mermaid();
        assert!(diagram.contains("isolated1[isolated1]"));
        assert!(diagram.contains("isolated2[isolated2]"));
        assert!(diagram.contains("Start([Start]) --> isolated1"));
        assert!(!diagram.contains("End([End])"));
    }

    #[test]
    fn test_to_mermaid_parallel_edges_with_multiple_targets() {
        // Test to_mermaid with parallel edges to 3+ targets
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("w1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("w2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("w3", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges(
            "start",
            vec!["w1".to_string(), "w2".to_string(), "w3".to_string()],
        );
        graph.set_entry_point("start");

        let diagram = graph.to_mermaid();
        assert!(diagram.contains("start ==> w1"));
        assert!(diagram.contains("start ==> w2"));
        assert!(diagram.contains("start ==> w3"));
    }

    #[test]
    fn test_to_mermaid_conditional_edges_with_many_routes() {
        // Test to_mermaid with conditional edges having 4+ routes
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("router", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("path1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("path2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("path3", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("path4", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("a".to_string(), "path1".to_string());
        routes.insert("b".to_string(), "path2".to_string());
        routes.insert("c".to_string(), "path3".to_string());
        routes.insert("d".to_string(), "path4".to_string());

        graph.add_conditional_edges("router", |_: &AgentState| "a".to_string(), routes);
        graph.set_entry_point("router");

        let diagram = graph.to_mermaid();
        assert!(diagram.contains("router -->|a| path1"));
        assert!(diagram.contains("router -->|b| path2"));
        assert!(diagram.contains("router -->|c| path3"));
        assert!(diagram.contains("router -->|d| path4"));
    }

    #[test]
    fn test_compile_parallel_edge_from_nonexistent_node() {
        // Test compilation fails when parallel edge references missing source node
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("target1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("target2", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges(
            "missing",
            vec!["target1".to_string(), "target2".to_string()],
        );
        graph.set_entry_point("target1");

        let result = graph.compile_with_merge();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_compile_conditional_edge_from_nonexistent_node() {
        // Test compilation fails when conditional edge references missing source node
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("target", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("go".to_string(), "target".to_string());

        graph.add_conditional_edges("missing", |_: &AgentState| "go".to_string(), routes);
        graph.set_entry_point("target");

        let result = graph.compile();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_validate_conditional_edge_route_to_missing_node() {
        // Test validation when conditional edge route points to missing node
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("decision", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("go".to_string(), "missing".to_string());

        graph.add_conditional_edges("decision", |_: &AgentState| "go".to_string(), routes);
        graph.set_entry_point("decision");

        // Compile should fail
        let result = graph.compile();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_validate_parallel_edge_target_missing() {
        // Test validation when parallel edge targets missing node
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("target1", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges("start", vec!["target1".to_string(), "missing".to_string()]);
        graph.set_entry_point("start");

        let result = graph.compile_with_merge();
        assert!(matches!(result, Err(Error::NodeNotFound(_))));
    }

    #[test]
    fn test_add_node_duplicate_names_overwrites() {
        // Test that adding node with duplicate name overwrites previous
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("duplicate", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("duplicate", |state| Box::pin(async move { Ok(state) })); // Overwrite

        assert_eq!(graph.nodes.len(), 1);
    }

    #[test]
    fn test_add_edge_duplicate_creates_multiple() {
        // Test that duplicate edges are allowed (creates multiple edge entries)
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("n1", "n2");
        graph.add_edge("n1", "n2"); // Duplicate

        assert_eq!(graph.edges.len(), 2);
    }

    #[test]
    fn test_validate_warns_empty_conditional_routes() {
        // Test validation warns when conditional edge has no routes
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("decision", |state| Box::pin(async move { Ok(state) }));

        let routes = HashMap::new(); // Empty routes
        graph.add_conditional_edges("decision", |_: &AgentState| "".to_string(), routes);
        graph.set_entry_point("decision");

        let warnings = graph.validate();
        assert!(warnings.iter().any(|w| w.contains("has no routes defined")));
    }

    #[test]
    fn test_compile_all_edges_to_end_valid() {
        // Test that graph with all paths leading to END compiles
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n3", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("n1", "n2");
        graph.add_edge("n1", "n3");
        graph.add_edge("n2", END);
        graph.add_edge("n3", END);
        graph.set_entry_point("n1");

        let result = graph.compile();
        assert!(result.is_ok());
    }

    #[test]
    fn test_to_mermaid_empty_conditional_routes() {
        // Test to_mermaid handles conditional edge with no routes
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("decision", |state| Box::pin(async move { Ok(state) }));

        let routes = HashMap::new();
        graph.add_conditional_edges("decision", |_: &AgentState| "".to_string(), routes);
        graph.set_entry_point("decision");

        let diagram = graph.to_mermaid();
        // Should generate diagram without conditional edges section
        assert!(diagram.contains("decision[decision]"));
    }

    #[test]
    fn test_find_reachable_nodes_all_routes_to_end() {
        // Test find_reachable_nodes when all edges point to END
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("only_node", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("only_node", END);
        graph.set_entry_point("only_node");

        let reachable = graph.find_reachable_nodes();
        assert_eq!(reachable.len(), 1);
        assert!(reachable.contains("only_node"));
    }

    #[test]
    fn test_has_cycles_conditional_edge_cycle() {
        // Test cycle detection through conditional edges
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("loop".to_string(), "n1".to_string());
        graph.add_conditional_edges("n2", |_: &AgentState| "loop".to_string(), routes);

        graph.add_edge("n1", "n2");

        assert!(graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_parallel_edge_cycle() {
        // Test cycle detection through parallel edges
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges("n1", vec!["n2".to_string()]);
        graph.add_parallel_edges("n2", vec!["n1".to_string()]);

        assert!(graph.has_cycles());
    }

    // ===== Additional Edge Case Tests for Coverage =====

    #[test]
    fn test_find_reachable_nodes_diamond_pattern() {
        // Test reachability with diamond pattern: start -> (a, b) -> end
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("end_node", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("start", "a");
        graph.add_edge("start", "b");
        graph.add_edge("a", "end_node");
        graph.add_edge("b", "end_node");
        graph.set_entry_point("start");

        let reachable = graph.find_reachable_nodes();
        assert_eq!(reachable.len(), 4);
        assert!(reachable.contains("start"));
        assert!(reachable.contains("a"));
        assert!(reachable.contains("b"));
        assert!(reachable.contains("end_node"));
    }

    #[test]
    fn test_find_reachable_nodes_with_cycle() {
        // Test reachability when graph has cycle
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n3", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("n1", "n2");
        graph.add_edge("n2", "n3");
        graph.add_edge("n3", "n1"); // Cycle
        graph.set_entry_point("n1");

        let reachable = graph.find_reachable_nodes();
        assert_eq!(reachable.len(), 3);
        assert!(reachable.contains("n1"));
        assert!(reachable.contains("n2"));
        assert!(reachable.contains("n3"));
    }

    #[test]
    fn test_find_reachable_nodes_mixed_edge_types_complex() {
        // Test reachability with all three edge types
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("simple_target", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("cond_a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("cond_b", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("parallel_1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("parallel_2", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("start", "simple_target");

        let mut routes = HashMap::new();
        routes.insert("a".to_string(), "cond_a".to_string());
        routes.insert("b".to_string(), "cond_b".to_string());
        graph.add_conditional_edges("simple_target", |_: &AgentState| "a".to_string(), routes);

        graph.add_parallel_edges(
            "cond_a",
            vec!["parallel_1".to_string(), "parallel_2".to_string()],
        );

        graph.set_entry_point("start");

        let reachable = graph.find_reachable_nodes();
        assert_eq!(reachable.len(), 6);
        assert!(reachable.contains("start"));
        assert!(reachable.contains("simple_target"));
        assert!(reachable.contains("cond_a"));
        assert!(reachable.contains("cond_b"));
        assert!(reachable.contains("parallel_1"));
        assert!(reachable.contains("parallel_2"));
    }

    #[test]
    fn test_has_cycles_no_cycle_with_end() {
        // Test that edges to END don't create false cycle detection
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("n1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("n2", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("n1", "n2");
        graph.add_edge("n2", END);
        graph.add_edge("n1", END);

        assert!(!graph.has_cycles());
    }

    #[test]
    fn test_has_cycles_with_unreachable_cycle() {
        // Test cycle detection when cycle exists but is unreachable
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("entry", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("isolated_a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("isolated_b", |state| Box::pin(async move { Ok(state) }));

        graph.add_edge("entry", END);
        // Create cycle in unreachable subgraph
        graph.add_edge("isolated_a", "isolated_b");
        graph.add_edge("isolated_b", "isolated_a");

        graph.set_entry_point("entry");

        // has_cycles checks all nodes, not just reachable ones
        assert!(graph.has_cycles());
    }

    #[test]
    fn test_validate_parallel_edge_with_empty_targets() {
        // Test validation of parallel edge with no targets
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_parallel_edges("start", vec![]);
        graph.set_entry_point("start");

        let warnings = graph.validate();
        // Empty parallel edges might be considered valid (rare but possible)
        // Main test is that it doesn't panic
        let _ = warnings;
    }

    #[test]
    fn test_validate_conditional_edge_with_end_only() {
        // Test conditional edge where all routes go to END
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("decision", |state| Box::pin(async move { Ok(state) }));

        let mut routes = HashMap::new();
        routes.insert("terminate".to_string(), END.to_string());
        graph.add_conditional_edges("decision", |_: &AgentState| "terminate".to_string(), routes);
        graph.set_entry_point("decision");

        let warnings = graph.validate();
        // Should not warn - END is a valid target
        assert!(!warnings
            .iter()
            .any(|w| w.contains("unreachable") || w.contains("missing")));

        let result = graph.compile_with_merge();
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_node_overwrites_duplicate_name_edge_case() {
        // Test that adding node with duplicate name overwrites previous
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node", |state| {
            Box::pin(async move {
                let mut new_state = state;
                new_state.iteration = 1;
                Ok(new_state)
            })
        });

        graph.add_node_from_fn("node", |state| {
            Box::pin(async move {
                let mut new_state = state;
                new_state.iteration = 2;
                Ok(new_state)
            })
        });

        assert_eq!(graph.nodes.len(), 1);
        // The second node should have replaced the first
    }

    #[test]
    fn test_multiple_entry_points_override() {
        // Test that setting entry point multiple times overrides previous
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("first", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("second", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("first");
        graph.set_entry_point("second");

        assert_eq!(graph.entry_point, Some("second".to_string()));
    }

    #[test]
    fn test_validate_returns_early_without_entry_point_edge_case() {
        // Test that validate returns early if no entry point (edge case version)
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("orphan", |state| Box::pin(async move { Ok(state) }));

        let warnings = graph.validate();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("No entry point"));
        // Should not contain warnings about unreachable nodes since validation stopped early
    }

    #[test]
    fn test_validate_all_nodes_unreachable_edge_case() {
        // Test validation when entry point is set but has no outgoing edges (edge case)
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("isolated_entry", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("unreachable2", |state| Box::pin(async move { Ok(state) }));

        graph.set_entry_point("isolated_entry");

        let warnings = graph.validate();
        // Should have warnings for 2 unreachable nodes
        assert_eq!(
            warnings
                .iter()
                .filter(|w| w.contains("unreachable"))
                .count(),
            2
        );
    }

    #[test]
    fn test_mixed_edge_types_validation() {
        // Test that mixing edge types on the same node causes validation error
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node3", |state| Box::pin(async move { Ok(state) }));

        // Add both simple edge and parallel edges from node1 (mixed types)
        graph.add_edge("node1", "node2"); // simple edge
        graph.add_parallel_edges("node1", vec!["node2".to_string(), "node3".to_string()]); // parallel edges

        graph.set_entry_point("node1");

        let result = graph.compile();

        // Should fail with validation error
        assert!(matches!(result, Err(Error::Validation(_))));

        if let Err(Error::Validation(msg)) = result {
            assert!(msg.contains("node1"));
            assert!(msg.contains("multiple edge types"));
            assert!(msg.contains("simple"));
            assert!(msg.contains("parallel"));
        }
    }

    #[test]
    fn test_mixed_conditional_and_simple_edges() {
        // Test that mixing conditional and simple edges fails
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node3", |state| Box::pin(async move { Ok(state) }));

        // Add both simple edge and conditional edge from node1
        graph.add_edge("node1", "node2");

        let mut routes = HashMap::new();
        routes.insert("next".to_string(), "node3".to_string());
        graph.add_conditional_edges("node1", |_state: &AgentState| "next".to_string(), routes);

        graph.set_entry_point("node1");

        let result = graph.compile();

        // Should fail with validation error
        assert!(matches!(result, Err(Error::Validation(_))));

        if let Err(Error::Validation(msg)) = result {
            assert!(msg.contains("node1"));
            assert!(msg.contains("multiple edge types"));
        }
    }

    #[test]
    fn test_no_mixed_edges_passes() {
        // Test that nodes with only one edge type compile successfully
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node3", |state| Box::pin(async move { Ok(state) }));

        // Only simple edges
        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node3");

        graph.set_entry_point("node1");

        let result = graph.compile();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parallel_edge_tracking() {
        // Test that has_parallel_edges flag is correctly set
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Initially should be false
        assert!(!graph.has_parallel_edges);

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker2", |state| Box::pin(async move { Ok(state) }));

        // Still false after adding nodes
        assert!(!graph.has_parallel_edges);

        // Add parallel edges
        graph.add_parallel_edges("start", vec!["worker1".to_string(), "worker2".to_string()]);

        // Now should be true
        assert!(graph.has_parallel_edges);
    }

    #[test]
    fn test_compile_fails_with_parallel_edges_without_merge() {
        // Issue 1: MergeableState Not Enforced
        // This test verifies that compile() fails with a helpful error message
        // when parallel edges are used, directing users to compile_with_merge()
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("worker2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("aggregator", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges("start", vec!["worker1".to_string(), "worker2".to_string()]);
        graph.add_edge("worker1", "aggregator");
        graph.add_edge("worker2", "aggregator");
        graph.add_edge("aggregator", END);
        graph.set_entry_point("start");

        // compile() should fail because parallel edges require MergeableState
        let result = graph.compile();
        assert!(result.is_err(), "compile() should fail with parallel edges");

        if let Err(Error::Validation(msg)) = result {
            assert!(
                msg.contains("parallel edges"),
                "Error should mention parallel edges: {msg}"
            );
            assert!(
                msg.contains("MergeableState"),
                "Error should mention MergeableState: {msg}"
            );
            assert!(
                msg.contains("compile_with_merge"),
                "Error should suggest compile_with_merge(): {msg}"
            );
        } else {
            panic!("Expected Validation error for parallel edges without merge");
        }
    }

    #[test]
    fn test_try_add_node_returns_error_on_duplicate() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // First add should succeed
        let result = graph.try_add_node(
            "node1",
            FunctionNode::new("node1_impl", |state: AgentState| {
                Box::pin(async move { Ok(state) })
            }),
        );
        assert!(result.is_ok());

        // Second add with same name should fail
        let result = graph.try_add_node(
            "node1",
            FunctionNode::new("node1_impl_v2", |state: AgentState| {
                Box::pin(async move { Ok(state) })
            }),
        );
        assert!(matches!(result, Err(Error::DuplicateNodeName(name)) if name == "node1"));
    }

    #[test]
    fn test_add_node_or_replace_does_not_error() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // First add
        graph.add_node_or_replace(
            "node1",
            FunctionNode::new("node1_impl", |state: AgentState| {
                Box::pin(async move { Ok(state) })
            }),
        );

        // Second add with same name - no error
        graph.add_node_or_replace(
            "node1",
            FunctionNode::new("node1_impl_v2", |state: AgentState| {
                Box::pin(async move { Ok(state) })
            }),
        );

        // Node should exist
        assert!(graph.nodes.contains_key("node1"));
    }

    #[test]
    fn test_strict_mode_flag() {
        let graph: StateGraph<AgentState> = StateGraph::new();
        assert!(!graph.is_strict());

        let strict_graph: StateGraph<AgentState> = StateGraph::new().strict();
        assert!(strict_graph.is_strict());
    }

    // ========================================================================
    // Node Configuration Tests
    // ========================================================================

    #[test]
    fn test_node_config_basic_creation() {
        use crate::introspection::NodeConfig;

        let config = NodeConfig::new("test_node", "llm.chat");
        assert_eq!(config.name, "test_node");
        assert_eq!(config.node_type, "llm.chat");
        assert_eq!(config.version, 1);
        assert!(config.config_hash.starts_with("sha256:"));
        assert!(config.updated_by.is_none());
    }

    #[test]
    fn test_node_config_with_config() {
        use crate::introspection::NodeConfig;

        let config = NodeConfig::new("llm", "llm.chat")
            .with_config(serde_json::json!({
                "system_prompt": "You are helpful.",
                "temperature": 0.7
            }))
            .with_updated_by("human");

        assert_eq!(config.system_prompt(), Some("You are helpful."));
        assert_eq!(config.temperature(), Some(0.7));
        assert_eq!(config.updated_by, Some("human".to_string()));
    }

    #[test]
    fn test_node_config_update_increments_version() {
        use crate::introspection::NodeConfig;

        let mut config =
            NodeConfig::new("llm", "llm.chat").with_config(serde_json::json!({"temperature": 0.7}));

        let original_hash = config.config_hash.clone();
        assert_eq!(config.version, 1);

        let previous = config.update(
            serde_json::json!({"temperature": 0.3}),
            Some("ab_test".to_string()),
        );

        assert_eq!(config.version, 2);
        assert_ne!(config.config_hash, original_hash);
        assert_eq!(config.updated_by, Some("ab_test".to_string()));
        assert_eq!(
            previous.get("temperature").and_then(|v| v.as_f64()),
            Some(0.7)
        );
    }

    #[test]
    fn test_set_node_config_in_graph() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Set a config
        graph.set_node_config(
            "llm_agent",
            serde_json::json!({
                "system_prompt": "You are a researcher.",
                "temperature": 0.5
            }),
            Some("human"),
        );

        // Retrieve and verify
        let config = graph.get_node_config("llm_agent").unwrap();
        assert_eq!(config.name, "llm_agent");
        assert_eq!(config.system_prompt(), Some("You are a researcher."));
        assert_eq!(config.temperature(), Some(0.5));
        assert_eq!(config.version, 1);
        assert_eq!(config.updated_by, Some("human".to_string()));
    }

    #[test]
    fn test_update_node_config_in_graph() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Set initial config
        graph.set_node_config("llm", serde_json::json!({"temperature": 0.7}), None);

        // Update the config
        let previous = graph
            .update_node_config(
                "llm",
                serde_json::json!({"temperature": 0.3, "max_tokens": 1000}),
                Some("ab_test".to_string()),
            )
            .unwrap();

        // Verify previous value
        assert_eq!(
            previous.get("temperature").and_then(|v| v.as_f64()),
            Some(0.7)
        );

        // Verify new value
        let config = graph.get_node_config("llm").unwrap();
        assert_eq!(config.temperature(), Some(0.3));
        assert_eq!(config.max_tokens(), Some(1000));
        assert_eq!(config.version, 2);
        assert_eq!(config.updated_by, Some("ab_test".to_string()));
    }

    #[test]
    fn test_update_node_config_not_found() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        let result = graph.update_node_config("nonexistent", serde_json::json!({}), None);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NodeNotFound(_)));
    }

    #[test]
    fn test_batch_update_configs() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Set initial configs
        graph.set_node_config("node_a", serde_json::json!({"prompt": "A"}), None);
        graph.set_node_config("node_b", serde_json::json!({"prompt": "B"}), None);

        // Batch update
        let mut updates = std::collections::HashMap::new();
        updates.insert(
            "node_a".to_string(),
            serde_json::json!({"prompt": "A-updated"}),
        );
        updates.insert(
            "node_b".to_string(),
            serde_json::json!({"prompt": "B-updated"}),
        );
        updates.insert("node_c".to_string(), serde_json::json!({"prompt": "C-new"}));

        let previous = graph.update_configs(updates, Some("batch_update".to_string()));

        // Verify previous values
        assert_eq!(
            previous
                .get("node_a")
                .and_then(|v| v.get("prompt"))
                .and_then(|v| v.as_str()),
            Some("A")
        );
        assert_eq!(
            previous
                .get("node_b")
                .and_then(|v| v.get("prompt"))
                .and_then(|v| v.as_str()),
            Some("B")
        );
        // node_c was new, so previous is Null
        assert_eq!(previous.get("node_c"), Some(&serde_json::Value::Null));

        // Verify new values
        assert_eq!(
            graph
                .get_node_config("node_a")
                .unwrap()
                .get_field("prompt")
                .and_then(|v| v.as_str()),
            Some("A-updated")
        );
        assert_eq!(graph.get_node_config("node_a").unwrap().version, 2);

        assert_eq!(
            graph
                .get_node_config("node_c")
                .unwrap()
                .get_field("prompt")
                .and_then(|v| v.as_str()),
            Some("C-new")
        );
        assert_eq!(graph.get_node_config("node_c").unwrap().version, 1);
    }

    #[test]
    fn test_has_node_config() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        assert!(!graph.has_node_config("llm"));

        graph.set_node_config("llm", serde_json::json!({}), None);

        assert!(graph.has_node_config("llm"));
    }

    #[test]
    fn test_remove_node_config() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.set_node_config("llm", serde_json::json!({"temp": 0.5}), None);
        assert!(graph.has_node_config("llm"));

        let removed = graph.remove_node_config("llm");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().temperature(), None); // was temp, not temperature

        assert!(!graph.has_node_config("llm"));
    }

    #[test]
    fn test_node_configs_in_manifest() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.set_entry_point("start");

        // Set some configs
        graph.set_node_config(
            "researcher",
            serde_json::json!({
                "system_prompt": "Research topics thoroughly"
            }),
            Some("human"),
        );

        // Get manifest
        let manifest = graph.manifest();

        // Verify configs are included
        assert!(!manifest.node_configs.is_empty());
        let researcher_config = manifest.node_configs.get("researcher").unwrap();
        assert_eq!(
            researcher_config.system_prompt(),
            Some("Research topics thoroughly")
        );
    }

    #[test]
    fn test_node_config_hash_consistency() {
        use crate::introspection::NodeConfig;

        let config1 = serde_json::json!({"a": 1, "b": 2});
        let config2 = serde_json::json!({"a": 1, "b": 2});

        let hash1 = NodeConfig::compute_hash(&config1);
        let hash2 = NodeConfig::compute_hash(&config2);

        assert_eq!(hash1, hash2);

        // Different config should have different hash
        let config3 = serde_json::json!({"a": 1, "b": 3});
        let hash3 = NodeConfig::compute_hash(&config3);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_get_all_node_configs() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.set_node_config("a", serde_json::json!({}), None);
        graph.set_node_config("b", serde_json::json!({}), None);
        graph.set_node_config("c", serde_json::json!({}), None);

        let configs = graph.get_all_node_configs();
        assert_eq!(configs.len(), 3);
        assert!(configs.contains_key("a"));
        assert!(configs.contains_key("b"));
        assert!(configs.contains_key("c"));
    }

    // =========================================================================
    // Interpreter Mode Tests
    // =========================================================================

    #[tokio::test]
    async fn test_execute_unvalidated_simple() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("node1", |mut state| {
            Box::pin(async move {
                state.add_message("node1 executed");
                Ok(state)
            })
        });

        graph.add_node_from_fn("node2", |mut state| {
            Box::pin(async move {
                state.add_message("node2 executed");
                Ok(state)
            })
        });

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", END);
        graph.set_entry_point("node1");

        // Execute without compile step
        let result = graph.execute_unvalidated(AgentState::new()).await.unwrap();

        assert_eq!(result.nodes_executed, vec!["node1", "node2"]);
        assert!(result
            .final_state
            .messages
            .iter()
            .any(|m| m.contains("node1")));
        assert!(result
            .final_state
            .messages
            .iter()
            .any(|m| m.contains("node2")));
    }

    #[tokio::test]
    async fn test_execute_unvalidated_conditional_edges() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("path_a", |mut state| {
            Box::pin(async move {
                state.add_message("path_a");
                Ok(state)
            })
        });
        graph.add_node_from_fn("path_b", |mut state| {
            Box::pin(async move {
                state.add_message("path_b");
                Ok(state)
            })
        });

        // Conditional edge based on message count
        graph.add_conditional_edges(
            "start",
            |state: &AgentState| {
                if state.messages.is_empty() {
                    "go_a".to_string()
                } else {
                    "go_b".to_string()
                }
            },
            vec![
                ("go_a".to_string(), "path_a".to_string()),
                ("go_b".to_string(), "path_b".to_string()),
            ]
            .into_iter()
            .collect(),
        );
        graph.add_edge("path_a", END);
        graph.add_edge("path_b", END);
        graph.set_entry_point("start");

        // Execute - should take path_a since messages is empty
        let result = graph.execute_unvalidated(AgentState::new()).await.unwrap();
        assert!(result
            .final_state
            .messages
            .iter()
            .any(|m| m.contains("path_a")));
        assert!(!result
            .final_state
            .messages
            .iter()
            .any(|m| m.contains("path_b")));
    }

    #[tokio::test]
    async fn test_execute_unvalidated_no_entry_point_fails() {
        let graph: StateGraph<AgentState> = StateGraph::new();

        // Should fail - no entry point
        let result = graph.execute_unvalidated(AgentState::new()).await;
        assert!(result.is_err());
        match result {
            Err(crate::error::Error::NoEntryPoint) => {}
            _ => panic!("Expected NoEntryPoint error"),
        }
    }

    #[tokio::test]
    async fn test_execute_unvalidated_missing_entry_node_fails() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.set_entry_point("nonexistent");

        // Should fail - entry point doesn't exist
        let result = graph.execute_unvalidated(AgentState::new()).await;
        assert!(result.is_err());
        match result {
            Err(crate::error::Error::NodeNotFound(name)) => {
                assert_eq!(name, "nonexistent");
            }
            _ => panic!("Expected NodeNotFound error"),
        }
    }

    #[test]
    fn test_structural_hash_consistency() {
        let mut graph1: StateGraph<AgentState> = StateGraph::new();
        graph1.add_node_from_fn("a", |s| Box::pin(async move { Ok(s) }));
        graph1.add_node_from_fn("b", |s| Box::pin(async move { Ok(s) }));
        graph1.add_edge("a", "b");
        graph1.add_edge("b", END);
        graph1.set_entry_point("a");

        let mut graph2: StateGraph<AgentState> = StateGraph::new();
        // Add in different order - hash should still be the same
        graph2.add_node_from_fn("b", |s| Box::pin(async move { Ok(s) }));
        graph2.add_node_from_fn("a", |s| Box::pin(async move { Ok(s) }));
        graph2.add_edge("b", END);
        graph2.add_edge("a", "b");
        graph2.set_entry_point("a");

        // Same structure should have same hash
        assert_eq!(graph1.structural_hash(), graph2.structural_hash());
    }

    #[test]
    fn test_structural_hash_differs_for_different_graphs() {
        let mut graph1: StateGraph<AgentState> = StateGraph::new();
        graph1.add_node_from_fn("a", |s| Box::pin(async move { Ok(s) }));
        graph1.add_edge("a", END);
        graph1.set_entry_point("a");

        let mut graph2: StateGraph<AgentState> = StateGraph::new();
        graph2.add_node_from_fn("a", |s| Box::pin(async move { Ok(s) }));
        graph2.add_node_from_fn("b", |s| Box::pin(async move { Ok(s) }));
        graph2.add_edge("a", "b");
        graph2.add_edge("b", END);
        graph2.set_entry_point("a");

        // Different structures should have different hashes
        assert_ne!(graph1.structural_hash(), graph2.structural_hash());
    }

    #[test]
    fn test_compile_delta_unchanged_structure() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("a", |s| Box::pin(async move { Ok(s) }));
        graph.add_edge("a", END);
        graph.set_entry_point("a");

        // First compile
        let compiled = graph.clone().compile().unwrap();
        let original_hash = compiled.structural_hash();

        // Compile delta with unchanged graph
        let recompiled = graph.compile_delta(&compiled).unwrap();

        // Hash should be the same
        assert_eq!(original_hash, recompiled.structural_hash());
    }

    #[test]
    fn test_compile_delta_changed_structure() {
        let mut graph: StateGraph<AgentState> = StateGraph::new();
        graph.add_node_from_fn("a", |s| Box::pin(async move { Ok(s) }));
        graph.add_edge("a", END);
        graph.set_entry_point("a");

        // First compile
        let compiled = graph.compile().unwrap();
        let original_hash = compiled.structural_hash();

        // Create different graph
        let mut graph2: StateGraph<AgentState> = StateGraph::new();
        graph2.add_node_from_fn("a", |s| Box::pin(async move { Ok(s) }));
        graph2.add_node_from_fn("b", |s| Box::pin(async move { Ok(s) }));
        graph2.add_edge("a", "b");
        graph2.add_edge("b", END);
        graph2.set_entry_point("a");

        // Compile delta should do full recompilation
        let recompiled = graph2.compile_delta(&compiled).unwrap();

        // Hash should be different
        assert_ne!(original_hash, recompiled.structural_hash());
    }

    // Runtime Validation Tests

    /// A mock node that uses LLM but is not optimizable (should trigger warning)
    struct NonOptimizableLLMNode;

    #[async_trait::async_trait]
    impl Node<AgentState> for NonOptimizableLLMNode {
        async fn execute(&self, state: AgentState) -> Result<AgentState> {
            Ok(state)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn may_use_llm(&self) -> bool {
            true // Reports LLM usage
        }

        fn is_optimizable(&self) -> bool {
            false // But is not optimizable - should trigger warning
        }
    }

    /// A mock node that uses LLM and is optimizable (no warning)
    struct OptimizableLLMNode;

    #[async_trait::async_trait]
    impl Node<AgentState> for OptimizableLLMNode {
        async fn execute(&self, state: AgentState) -> Result<AgentState> {
            Ok(state)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn may_use_llm(&self) -> bool {
            true
        }

        fn is_optimizable(&self) -> bool {
            true // Properly optimizable
        }
    }

    #[test]
    fn test_validate_non_optimizable_llm_warning() {
        // Tests that validate() warns about LLM nodes that aren't optimizable
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        let node: BoxedNode<AgentState> = Arc::new(NonOptimizableLLMNode);
        graph.add_boxed_node("llm_node", node);
        graph.add_edge("llm_node", END);
        graph.set_entry_point("llm_node");

        let warnings = graph.validate();

        // Should have warning about non-optimizable LLM node
        assert!(
            warnings.iter().any(|w| w.contains("not optimizable")),
            "Expected warning about non-optimizable LLM node, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validate_optimizable_llm_no_warning() {
        // Tests that validate() does NOT warn about optimizable LLM nodes
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        let node: BoxedNode<AgentState> = Arc::new(OptimizableLLMNode);
        graph.add_boxed_node("llm_node", node);
        graph.add_edge("llm_node", END);
        graph.set_entry_point("llm_node");

        let warnings = graph.validate();

        // Should NOT have warning about non-optimizable LLM node
        assert!(
            !warnings.iter().any(|w| w.contains("not optimizable")),
            "Should not warn about optimizable LLM node, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validate_regular_node_no_llm_warning() {
        // Tests that validate() does NOT warn about regular nodes that don't use LLM
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        // Function nodes default to may_use_llm() = false
        graph.add_node_from_fn("regular_node", |state| Box::pin(async move { Ok(state) }));
        graph.add_edge("regular_node", END);
        graph.set_entry_point("regular_node");

        let warnings = graph.validate();

        // Should NOT have warning about non-optimizable LLM node
        assert!(
            !warnings.iter().any(|w| w.contains("not optimizable")),
            "Should not warn about regular nodes, got: {:?}",
            warnings
        );
    }
}
