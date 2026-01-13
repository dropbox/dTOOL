// Imports for production code (UnifiedMcpServer, etc.) below the unit test module
#[cfg(feature = "mcp-server")]
use super::{McpSelfDocServer, SCHEMA_VERSION};
#[cfg(feature = "mcp-server")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "mcp-server")]
use std::sync::Arc;

#[cfg(test)]
mod unit_tests {
    use super::super::*;
    use crate::introspection::{
        CapabilityManifest, EdgeManifest, GraphManifest, GraphMetadata, NodeManifest, NodeType,
        ToolManifest,
    };
    use crate::platform_registry::{
        AppArchitectureBuilder, ArchitectureGraphInfo, Dependency, FeatureUsage, PlatformRegistry,
    };

    fn create_test_introspection() -> GraphIntrospection {
        let manifest = GraphManifest::builder()
            .graph_name("Test Agent")
            .entry_point("input")
            .add_node(
                "input",
                NodeManifest::new("input", NodeType::Function)
                    .with_description("Process user input"),
            )
            .add_node(
                "reasoning",
                NodeManifest::new("reasoning", NodeType::Agent)
                    .with_description("LLM-based reasoning"),
            )
            .add_node(
                "output",
                NodeManifest::new("output", NodeType::Function).with_description("Format output"),
            )
            .add_edge("input", EdgeManifest::simple("input", "reasoning"))
            .add_edge("reasoning", EdgeManifest::simple("reasoning", "__end__"))
            .metadata(GraphMetadata::default())
            .build()
            .unwrap();

        let platform = PlatformRegistry::discover();

        let mut builder = AppArchitectureBuilder::new().graph_structure(ArchitectureGraphInfo {
            name: Some("Test Agent".to_string()),
            node_count: 3,
            edge_count: 2,
            entry_point: "input".to_string(),
            node_names: vec![
                "input".to_string(),
                "reasoning".to_string(),
                "output".to_string(),
            ],
            has_cycles: false,
            has_conditional_edges: false,
            has_parallel_edges: false,
        });
        builder.add_feature(FeatureUsage::new(
            "StateGraph",
            "core",
            "Core orchestration",
        ));
        builder.add_dependency(
            Dependency::new("dashflow", "Core framework")
                .with_version("1.11.3")
                .dashflow(),
        );
        let architecture = builder.build();

        let mut capabilities = CapabilityManifest::default();
        capabilities
            .tools
            .push(ToolManifest::new("search", "Search the web"));

        GraphIntrospection {
            manifest,
            platform,
            architecture,
            capabilities,
        }
    }

    #[test]
    fn test_help_level_from_arg() {
        assert_eq!(HelpLevel::from_arg("--help"), Some(HelpLevel::Brief));
        assert_eq!(HelpLevel::from_arg("-h"), Some(HelpLevel::Brief));
        assert_eq!(HelpLevel::from_arg("--help-more"), Some(HelpLevel::More));
        assert_eq!(
            HelpLevel::from_arg("--help-implementation"),
            Some(HelpLevel::Implementation)
        );
        assert_eq!(
            HelpLevel::from_arg("--help-impl"),
            Some(HelpLevel::Implementation)
        );
        assert_eq!(HelpLevel::from_arg("--unknown"), None);
    }

    #[test]
    fn test_help_generator_brief() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let help = generator.generate(HelpLevel::Brief);

        assert!(help.contains("Test Agent"));
        assert!(help.contains("--help"));
        assert!(help.contains("--help-more"));
    }

    #[test]
    fn test_help_generator_more() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let help = generator.generate(HelpLevel::More);

        assert!(help.contains("ARCHITECTURE"));
        assert!(help.contains("NODES"));
        assert!(help.contains("DASHFLOW FEATURES"));
    }

    #[test]
    fn test_help_generator_implementation() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let help = generator.generate(HelpLevel::Implementation);

        assert!(help.contains("IMPLEMENTATION DETAILS"));
        assert!(help.contains("NODE VERSIONS"));
        assert!(help.contains("DEPENDENCIES"));
    }

    #[test]
    fn test_help_generator_custom_app_info() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection)
            .with_app_name("Custom Agent")
            .with_app_version("2.0.0")
            .with_app_description("A custom description");

        let help = generator.generate(HelpLevel::Brief);

        assert!(help.contains("Custom Agent"));
        assert!(help.contains("v2.0.0"));
        assert!(help.contains("A custom description"));
    }

    #[test]
    fn test_mcp_server_about_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.about_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert_eq!(response.protocol, PROTOCOL_VERSION);
        assert_eq!(response.name, "Test Agent");
        assert!(!response.capabilities.is_empty());
    }

    #[test]
    fn test_mcp_server_capabilities_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.capabilities_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert!(!response.tools.is_empty());
        assert!(!response.nodes.is_empty());
    }

    #[test]
    fn test_mcp_server_architecture_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.architecture_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert_eq!(response.graph.entry_point, "input");
        assert!(response.graph.node_count > 0);
        // Verify new optional fields are populated
        assert_eq!(response.graph.has_cycles, Some(false));
        assert_eq!(response.graph.has_parallel_paths, Some(false));
    }

    #[test]
    fn test_mcp_server_implementation_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.implementation_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert!(!response.dashflow_version.is_empty());
    }

    #[test]
    fn test_mcp_server_query_nodes() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.handle_query(&McpQueryRequest {
            question: "What nodes do you have?".to_string(),
        });

        assert!(
            response.answer.contains("node"),
            "Should contain node: {}",
            response.answer
        );
        assert!(response.confidence > 0.5);
    }

    #[test]
    fn test_mcp_server_query_tools() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.handle_query(&McpQueryRequest {
            question: "What tools are available?".to_string(),
        });

        assert!(
            response.answer.contains("tool"),
            "Should contain tool: {}",
            response.answer
        );
    }

    #[test]
    fn test_mcp_server_query_how_work() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.handle_query(&McpQueryRequest {
            question: "How does this work?".to_string(),
        });

        assert!(response.answer.contains("DashFlow"));
    }

    #[test]
    fn test_mcp_server_query_version() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.handle_query(&McpQueryRequest {
            question: "What version?".to_string(),
        });

        assert!(response.answer.contains("version"));
        assert!(response.confidence > 0.9);
    }

    #[test]
    fn test_mcp_server_query_unknown() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.handle_query(&McpQueryRequest {
            question: "Random unrelated question".to_string(),
        });

        // Should provide helpful guidance
        assert!(
            response.answer.contains("Try"),
            "Should contain Try: {}",
            response.answer
        );
        assert!(response.confidence < 0.7);
    }

    #[test]
    fn test_mcp_server_custom_app_info() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080)
            .with_app_name("Custom App")
            .with_app_version("3.0.0");

        let response = server.about_response();
        assert_eq!(response.name, "Custom App");
        assert_eq!(response.version, "3.0.0");
    }

    #[test]
    fn test_mcp_response_serialization() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        // Test that all responses serialize correctly and include schema_version
        let about = server.about_response();
        let about_json = serde_json::to_string(&about).unwrap();
        assert!(about_json.contains("\"schema_version\""));
        assert!(about_json.contains("\"name\""));

        let caps = server.capabilities_response();
        let caps_json = serde_json::to_string(&caps).unwrap();
        assert!(caps_json.contains("\"schema_version\""));
        assert!(caps_json.contains("\"tools\""));

        let arch = server.architecture_response();
        let arch_json = serde_json::to_string(&arch).unwrap();
        assert!(arch_json.contains("\"schema_version\""));
        assert!(arch_json.contains("\"graph\""));

        let impl_resp = server.implementation_response();
        let impl_json = serde_json::to_string(&impl_resp).unwrap();
        assert!(impl_json.contains("\"schema_version\""));
        assert!(impl_json.contains("\"dashflow_version\""));
    }

    #[test]
    fn test_schema_version_constants() {
        // Verify schema version is valid semver format
        assert!(SCHEMA_VERSION.split('.').count() == 3);
        for part in SCHEMA_VERSION.split('.') {
            assert!(part.parse::<u32>().is_ok());
        }

        // Verify protocol constants
        assert!(PROTOCOL_ID.starts_with("dashflow"));
        assert!(PROTOCOL_VERSION.contains(PROTOCOL_ID));
    }

    #[test]
    fn test_node_metadata_keys() {
        // Verify standard metadata keys are defined
        assert_eq!(node_metadata_keys::VERSION, "version");
        assert_eq!(node_metadata_keys::AUTHOR, "author");
        assert_eq!(node_metadata_keys::CATEGORY, "category");
        assert_eq!(node_metadata_keys::TAGS, "tags");
        assert_eq!(node_metadata_keys::DEPRECATED, "deprecated");
    }

    #[test]
    fn test_graph_metadata_keys() {
        // Verify standard graph metadata keys are defined
        assert_eq!(graph_metadata_keys::DESCRIPTION, "description");
        assert_eq!(graph_metadata_keys::VERSION, "version");
        assert_eq!(graph_metadata_keys::AUTHOR, "author");
        assert_eq!(graph_metadata_keys::LICENSE, "license");
    }

    #[test]
    fn test_node_info_with_version_and_metadata() {
        // Create a node with version metadata
        let mut node_manifest =
            NodeManifest::new("test_node", NodeType::Function).with_description("A test node");
        node_manifest.metadata.insert(
            node_metadata_keys::VERSION.to_string(),
            serde_json::json!("2.1.0"),
        );
        node_manifest.metadata.insert(
            node_metadata_keys::CATEGORY.to_string(),
            serde_json::json!("processing"),
        );

        let manifest = GraphManifest::builder()
            .graph_name("Test Graph")
            .entry_point("test_node")
            .add_node("test_node", node_manifest)
            .add_edge("test_node", EdgeManifest::simple("test_node", "__end__"))
            .metadata(GraphMetadata::default())
            .build()
            .unwrap();

        let platform = PlatformRegistry::discover();
        let builder = AppArchitectureBuilder::new().graph_structure(ArchitectureGraphInfo {
            name: Some("Test Graph".to_string()),
            node_count: 1,
            edge_count: 1,
            entry_point: "test_node".to_string(),
            node_names: vec!["test_node".to_string()],
            has_cycles: false,
            has_conditional_edges: false,
            has_parallel_edges: false,
        });
        let architecture = builder.build();
        let capabilities = CapabilityManifest::default();

        let introspection = GraphIntrospection {
            manifest,
            platform,
            architecture,
            capabilities,
        };

        let server = McpSelfDocServer::new(introspection, 8080);
        let caps_response = server.capabilities_response();

        // Verify node has version and metadata
        let node = caps_response
            .nodes
            .iter()
            .find(|n| n.name == "test_node")
            .unwrap();
        assert_eq!(node.version, Some("2.1.0".to_string()));
        assert!(node.metadata.is_some());
        let metadata = node.metadata.as_ref().unwrap();
        assert_eq!(
            metadata.get("category"),
            Some(&serde_json::json!("processing"))
        );
    }

    // ========================================================================
    // CLI Integration Tests
    // ========================================================================

    #[test]
    fn test_help_level_from_args() {
        // Test with --help
        let args = vec!["myapp", "--help"];
        assert_eq!(HelpLevel::from_args(&args), Some(HelpLevel::Brief));

        // Test with -h
        let args = vec!["myapp", "-h"];
        assert_eq!(HelpLevel::from_args(&args), Some(HelpLevel::Brief));

        // Test with --help-more
        let args = vec!["myapp", "--help-more"];
        assert_eq!(HelpLevel::from_args(&args), Some(HelpLevel::More));

        // Test with --help-implementation
        let args = vec!["myapp", "--help-implementation"];
        assert_eq!(HelpLevel::from_args(&args), Some(HelpLevel::Implementation));

        // Test with --help-impl shorthand
        let args = vec!["myapp", "--help-impl"];
        assert_eq!(HelpLevel::from_args(&args), Some(HelpLevel::Implementation));

        // Test with no help flags
        let args = vec!["myapp", "--verbose", "--config", "file.toml"];
        assert_eq!(HelpLevel::from_args(&args), None);

        // Test with help flag among other args
        let args = vec!["myapp", "--verbose", "--help", "--debug"];
        assert_eq!(HelpLevel::from_args(&args), Some(HelpLevel::Brief));
    }

    #[test]
    fn test_help_level_is_help_requested() {
        assert!(HelpLevel::is_help_requested(["app", "--help"]));
        assert!(HelpLevel::is_help_requested(["app", "-h"]));
        assert!(HelpLevel::is_help_requested(["app", "--help-more"]));
        assert!(HelpLevel::is_help_requested([
            "app",
            "--help-implementation"
        ]));
        assert!(!HelpLevel::is_help_requested(["app", "--verbose"]));
        let empty: Vec<String> = vec![];
        assert!(!HelpLevel::is_help_requested(&empty));
    }

    #[test]
    fn test_cli_help_config() {
        let config = CliHelpConfig::new()
            .with_app_name("Test App")
            .with_app_version("2.0.0")
            .with_app_description("A test application");

        assert_eq!(config.app_name, Some("Test App".to_string()));
        assert_eq!(config.app_version, Some("2.0.0".to_string()));
        assert_eq!(
            config.app_description,
            Some("A test application".to_string())
        );
        assert!(!config.output_to_stderr);
    }

    #[test]
    fn test_cli_help_config_stderr() {
        let config = CliHelpConfig::new().output_to_stderr();
        assert!(config.output_to_stderr);
    }

    #[test]
    fn test_cli_help_result() {
        let continue_result = CliHelpResult::Continue;
        assert!(continue_result.should_continue());
        assert!(!continue_result.should_exit());

        let displayed_result = CliHelpResult::Displayed(HelpLevel::Brief);
        assert!(!displayed_result.should_continue());
        assert!(displayed_result.should_exit());
    }

    #[test]
    fn test_process_cli_help_no_help_flag() {
        let introspection = create_test_introspection();
        let args = vec!["myapp", "--verbose", "--config", "file.toml"];

        let result = process_cli_help(args, introspection, None);
        assert_eq!(result, CliHelpResult::Continue);
    }

    #[test]
    fn test_process_cli_help_with_custom_config() {
        let introspection = create_test_introspection();
        let config = CliHelpConfig::new()
            .with_app_name("Custom App")
            .with_app_version("3.0.0");

        // Use a non-help arg to verify we get Continue
        let args = vec!["myapp", "--verbose"];
        let result = process_cli_help(args, introspection, Some(config));
        assert_eq!(result, CliHelpResult::Continue);
    }

    #[test]
    fn test_cli_help_config_default() {
        let config = CliHelpConfig::default();
        assert!(config.app_name.is_none());
        assert!(config.app_version.is_none());
        assert!(config.app_description.is_none());
        assert!(!config.output_to_stderr);
    }

    // ========================================================================
    // Enhanced Query Interface Tests
    // ========================================================================

    #[test]
    fn test_query_entry_point() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        // Test various entry point query patterns
        for question in &[
            "Where does it start?",
            "What is the entry point?",
            "First node?",
            "Where does execution start?",
            "Starting point?",
            "beginning",
        ] {
            let response = server.handle_query(&McpQueryRequest {
                question: question.to_string(),
            });
            assert!(
                response.answer.contains("input") || response.answer.contains("start"),
                "Question '{}' should mention entry point: {}",
                question,
                response.answer
            );
            assert!(response.confidence >= 0.8);
        }
    }

    #[test]
    fn test_query_terminal_nodes() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        // Test terminal node query patterns
        for question in &[
            "Where does it end?",
            "Terminal nodes?",
            "Exit points?",
            "final node",
        ] {
            let response = server.handle_query(&McpQueryRequest {
                question: question.to_string(),
            });
            assert!(
                response.answer.contains("terminal") || response.answer.contains("end"),
                "Question '{}' should mention terminal: {}",
                question,
                response.answer
            );
        }
    }

    #[test]
    fn test_query_edges() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        // Test edge/connection query patterns
        for question in &[
            "What are the edges?",
            "How are nodes connected?",
            "connections",
            "graph structure",
        ] {
            let response = server.handle_query(&McpQueryRequest {
                question: question.to_string(),
            });
            assert!(
                response.answer.contains("edge") || response.answer.contains("connect"),
                "Question '{}' should mention edges: {}",
                question,
                response.answer
            );
        }
    }

    #[test]
    fn test_query_features() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        let response = server.handle_query(&McpQueryRequest {
            question: "What features does this app use?".to_string(),
        });
        assert!(
            response.answer.contains("feature") || response.answer.contains("StateGraph"),
            "Should mention features: {}",
            response.answer
        );
    }

    #[test]
    fn test_query_specific_node() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        // Query about the "reasoning" node
        let response = server.handle_query(&McpQueryRequest {
            question: "Tell me about the reasoning node".to_string(),
        });
        assert!(
            response.answer.contains("reasoning"),
            "Should mention the reasoning node: {}",
            response.answer
        );
        assert!(response.confidence >= 0.9);
        assert!(!response.sources.is_empty());
    }

    #[test]
    fn test_query_description() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        for question in &["What are you?", "Who are you?", "Describe yourself"] {
            let response = server.handle_query(&McpQueryRequest {
                question: question.to_string(),
            });
            assert!(
                response.answer.contains("I am") || response.answer.contains("Test Agent"),
                "Question '{}' should describe the app: {}",
                question,
                response.answer
            );
        }
    }

    #[test]
    fn test_query_count() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        for question in &["How many nodes?", "Count everything", "total", "number of"] {
            let response = server.handle_query(&McpQueryRequest {
                question: question.to_string(),
            });
            assert!(
                response.answer.contains("node") || response.answer.contains("Statistics"),
                "Question '{}' should mention statistics: {}",
                question,
                response.answer
            );
        }
    }

    #[test]
    fn test_query_overview() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        for question in &["overview", "summary", "explain this app"] {
            let response = server.handle_query(&McpQueryRequest {
                question: question.to_string(),
            });
            assert!(
                response.answer.contains("DashFlow"),
                "Question '{}' should provide overview: {}",
                question,
                response.answer
            );
        }
    }

    #[test]
    fn test_query_fallback_includes_examples() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        let response = server.handle_query(&McpQueryRequest {
            question: "xyzzy foobar gibberish".to_string(),
        });

        // Fallback should include helpful suggestions and node examples
        assert!(response.answer.contains("Try"));
        assert!(response.confidence < 0.5);
    }

    #[test]
    fn test_query_tools_empty() {
        // Create introspection with no tools
        let manifest = GraphManifest::builder()
            .graph_name("No Tools App")
            .entry_point("start")
            .add_node(
                "start",
                NodeManifest::new("start", NodeType::Function).with_description("Start node"),
            )
            .add_edge("start", EdgeManifest::simple("start", "__end__"))
            .metadata(GraphMetadata::default())
            .build()
            .unwrap();

        let platform = PlatformRegistry::discover();
        let builder = AppArchitectureBuilder::new().graph_structure(ArchitectureGraphInfo {
            name: Some("No Tools App".to_string()),
            node_count: 1,
            edge_count: 1,
            entry_point: "start".to_string(),
            node_names: vec!["start".to_string()],
            has_cycles: false,
            has_conditional_edges: false,
            has_parallel_edges: false,
        });
        let architecture = builder.build();
        let capabilities = CapabilityManifest::default(); // No tools

        let introspection = GraphIntrospection {
            manifest,
            platform,
            architecture,
            capabilities,
        };

        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.handle_query(&McpQueryRequest {
            question: "What tools are available?".to_string(),
        });

        assert!(response.answer.contains("no explicitly registered tools"));
    }

    #[test]
    fn test_query_capabilities_shorthand() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        // "capabilit" matches "capabilities" pattern
        let response = server.handle_query(&McpQueryRequest {
            question: "capabilities".to_string(),
        });
        assert!(response.answer.contains("tool"));
    }

    #[test]
    fn test_query_case_insensitive() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        // Test case insensitivity
        let lower = server.handle_query(&McpQueryRequest {
            question: "what nodes do you have".to_string(),
        });
        let upper = server.handle_query(&McpQueryRequest {
            question: "WHAT NODES DO YOU HAVE".to_string(),
        });
        let mixed = server.handle_query(&McpQueryRequest {
            question: "WhAt NoDeS dO yOu HaVe".to_string(),
        });

        assert_eq!(lower.confidence, upper.confidence);
        assert_eq!(lower.confidence, mixed.confidence);
    }

    // ========================================================================
    // Node Drill-Down Endpoint Tests
    // ========================================================================

    #[test]
    fn test_nodes_list_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.nodes_list_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert_eq!(response.total_count, response.nodes.len());
        assert!(response.total_count > 0);

        // Verify entry point is marked
        let entry_node = response.nodes.iter().find(|n| n.is_entry_point);
        assert!(entry_node.is_some());
        assert_eq!(entry_node.unwrap().name, "input");

        // Verify terminal nodes are marked (reasoning goes to __end__ in test)
        let terminal_nodes: Vec<_> = response.nodes.iter().filter(|n| n.is_terminal).collect();
        assert!(terminal_nodes.iter().any(|n| n.name == "reasoning"));
    }

    #[test]
    fn test_nodes_list_serialization() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.nodes_list_response();

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"nodes\""));
        assert!(json.contains("\"total_count\""));
        assert!(json.contains("\"is_entry_point\""));
        assert!(json.contains("\"is_terminal\""));
    }

    #[test]
    fn test_node_detail_response_existing() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        let response = server.node_detail_response("input");
        assert!(response.is_some());

        let detail = response.unwrap();
        assert_eq!(detail.schema_version, SCHEMA_VERSION);
        assert_eq!(detail.name, "input");
        assert!(detail.is_entry_point);
        assert!(!detail.is_terminal);
        assert!(detail.description.is_some());
    }

    #[test]
    fn test_node_detail_response_nonexistent() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        let response = server.node_detail_response("nonexistent_node");
        assert!(response.is_none());
    }

    #[test]
    fn test_node_detail_edges() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        // Test middle node has both incoming and outgoing edges
        let response = server.node_detail_response("reasoning");
        assert!(response.is_some());

        let detail = response.unwrap();
        assert!(
            !detail.incoming_edges.is_empty(),
            "reasoning should have incoming edges"
        );
        assert!(
            !detail.outgoing_edges.is_empty(),
            "reasoning should have outgoing edges"
        );
    }

    #[test]
    fn test_node_detail_serialization() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        let response = server.node_detail_response("input").unwrap();
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"node_type\""));
        assert!(json.contains("\"incoming_edges\""));
        assert!(json.contains("\"outgoing_edges\""));
        assert!(json.contains("\"is_entry_point\""));
    }

    #[test]
    fn test_features_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.features_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert_eq!(response.total_count, response.features.len());

        // All features should be marked as enabled
        for feature in &response.features {
            assert!(feature.enabled);
            assert!(!feature.name.is_empty());
            assert!(!feature.description.is_empty());
        }
    }

    #[test]
    fn test_features_response_serialization() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.features_response();

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"features\""));
        assert!(json.contains("\"total_count\""));
    }

    // ========================================================================
    // Dependencies Endpoint Tests
    // ========================================================================

    #[test]
    fn test_dependencies_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.dependencies_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert_eq!(response.total_count, response.dependencies.len());
        assert_eq!(
            response.total_count,
            response.dashflow_count + response.external_count
        );
    }

    #[test]
    fn test_dependencies_response_counts() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.dependencies_response();

        // Verify counts match filtered results
        let dashflow_deps: Vec<_> = response
            .dependencies
            .iter()
            .filter(|d| d.is_dashflow)
            .collect();
        let external_deps: Vec<_> = response
            .dependencies
            .iter()
            .filter(|d| !d.is_dashflow)
            .collect();

        assert_eq!(response.dashflow_count, dashflow_deps.len());
        assert_eq!(response.external_count, external_deps.len());
    }

    #[test]
    fn test_dependencies_response_serialization() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.dependencies_response();

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"dependencies\""));
        assert!(json.contains("\"total_count\""));
        assert!(json.contains("\"dashflow_count\""));
        assert!(json.contains("\"external_count\""));
    }

    #[test]
    fn test_dependency_info_fields() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.dependencies_response();

        // All dependencies should have required fields
        for dep in &response.dependencies {
            assert!(!dep.name.is_empty());
            assert!(!dep.version.is_empty());
            assert!(!dep.purpose.is_empty());
        }
    }

    // ========================================================================
    // Edges Endpoint Tests
    // ========================================================================

    #[test]
    fn test_edges_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.edges_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert_eq!(response.total_count, response.edges.len());
        assert!(response.total_count > 0);
    }

    #[test]
    fn test_edges_response_conditional_count() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.edges_response();

        // Verify conditional count matches actual conditional edges
        let conditional_edges: Vec<_> =
            response.edges.iter().filter(|e| e.is_conditional).collect();

        assert_eq!(response.conditional_count, conditional_edges.len());
    }

    #[test]
    fn test_edges_response_serialization() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.edges_response();

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"edges\""));
        assert!(json.contains("\"total_count\""));
        assert!(json.contains("\"conditional_count\""));
    }

    #[test]
    fn test_edge_info_fields() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.edges_response();

        // All edges should have required fields
        for edge in &response.edges {
            assert!(!edge.from.is_empty());
            assert!(!edge.to.is_empty());
            // is_conditional is always present (bool)
            // condition is optional, so we don't check it
        }
    }

    #[test]
    fn test_edges_from_manifest() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.edges_response();

        // Verify edges match what we expect from the test manifest
        // Test manifest has: input -> reasoning -> __end__
        let has_input_to_reasoning = response
            .edges
            .iter()
            .any(|e| e.from == "input" && e.to == "reasoning");
        let has_reasoning_to_end = response
            .edges
            .iter()
            .any(|e| e.from == "reasoning" && e.to == "__end__");

        assert!(
            has_input_to_reasoning,
            "Should have edge from input to reasoning"
        );
        assert!(
            has_reasoning_to_end,
            "Should have edge from reasoning to __end__"
        );
    }

    // ========================================================================
    // Tool Registration Tests
    // ========================================================================

    #[test]
    fn test_server_with_tool() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080)
            .with_tool(ToolManifest::new("custom_tool", "A custom tool"));

        let about = server.about_response();
        assert!(about.capabilities.contains(&"custom_tool".to_string()));

        let caps = server.capabilities_response();
        assert!(caps.tools.iter().any(|t| t.name == "custom_tool"));
    }

    #[test]
    fn test_server_with_tools() {
        let introspection = create_test_introspection();
        let tools = vec![
            ToolManifest::new("tool_a", "First tool"),
            ToolManifest::new("tool_b", "Second tool"),
        ];
        let server = McpSelfDocServer::new(introspection, 8080).with_tools(tools);

        let about = server.about_response();
        assert!(about.capabilities.contains(&"tool_a".to_string()));
        assert!(about.capabilities.contains(&"tool_b".to_string()));
    }

    #[test]
    fn test_server_with_tool_parameters() {
        let introspection = create_test_introspection();
        // Use unique tool name to avoid conflict with test introspection's "search" tool
        let tool = ToolManifest::new("web_search", "Search the web for information")
            .with_category("web")
            .with_parameter("query", "string", "Search query", true)
            .with_parameter("limit", "number", "Max results", false);

        let server = McpSelfDocServer::new(introspection, 8080).with_tool(tool);

        let caps = server.capabilities_response();
        let search_tool = caps.tools.iter().find(|t| t.name == "web_search");
        assert!(search_tool.is_some());

        let search = search_tool.unwrap();
        assert!(search
            .description
            .contains("Search the web for information"));
        // Input schema should contain our parameters
        let schema = &search.input_schema;
        assert!(schema.get("properties").is_some());
    }

    #[test]
    fn test_tools_combined_with_introspection_tools() {
        // Test that additional tools are combined with introspection tools
        let introspection = create_test_introspection();
        let initial_tool_count = introspection.capabilities.tools.len();

        let server = McpSelfDocServer::new(introspection, 8080)
            .with_tool(ToolManifest::new("extra_tool", "Extra tool"));

        let caps = server.capabilities_response();
        assert_eq!(caps.tools.len(), initial_tool_count + 1);
    }

    #[test]
    fn test_tools_query_with_registered_tools() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080)
            .with_tool(ToolManifest::new("my_tool", "My custom tool"));

        let query = McpQueryRequest {
            question: "What tools are available?".to_string(),
        };
        let response = server.handle_query(&query);

        assert!(response.answer.contains("my_tool"));
    }

    #[test]
    fn test_count_query_includes_registered_tools() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080)
            .with_tool(ToolManifest::new("tool_1", "Tool 1"))
            .with_tool(ToolManifest::new("tool_2", "Tool 2"));

        let query = McpQueryRequest {
            question: "How many tools?".to_string(),
        };
        let response = server.handle_query(&query);

        // Should mention at least 2 tools (plus any from introspection)
        assert!(response.answer.contains("tool"));
    }

    #[test]
    fn test_with_tool_chaining() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080)
            .with_app_name("Test App")
            .with_tool(ToolManifest::new("tool1", "Tool 1"))
            .with_app_version("2.0.0")
            .with_tool(ToolManifest::new("tool2", "Tool 2"));

        let about = server.about_response();
        assert_eq!(about.name, "Test App");
        assert_eq!(about.version, "2.0.0");
        assert!(about.capabilities.contains(&"tool1".to_string()));
        assert!(about.capabilities.contains(&"tool2".to_string()));
    }

    // ========================================================================
    // App Introspection Enhancement Tests
    // ========================================================================

    #[test]
    fn test_tools_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.tools_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert_eq!(response.total_count, response.tools.len());
    }

    #[test]
    fn test_tools_response_with_registered_tools() {
        let introspection = create_test_introspection();
        let initial_count = introspection.capabilities.tools.len();

        let server = McpSelfDocServer::new(introspection, 8080)
            .with_tool(ToolManifest::new("extra_tool", "Extra tool").with_category("utility"));

        let response = server.tools_response();

        // Should include both introspection tools and registered tools
        assert_eq!(response.total_count, initial_count + 1);

        // Should have the extra tool
        assert!(response.tools.iter().any(|t| t.name == "extra_tool"));
    }

    #[test]
    fn test_tools_response_categories() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080)
            .with_tool(ToolManifest::new("tool_a", "Tool A").with_category("web"))
            .with_tool(ToolManifest::new("tool_b", "Tool B").with_category("filesystem"))
            .with_tool(ToolManifest::new("tool_c", "Tool C").with_category("web"));

        let response = server.tools_response();

        // Categories should be unique and sorted
        assert!(response.categories.contains(&"web".to_string()));
        assert!(response.categories.contains(&"filesystem".to_string()));

        // Verify deduplication
        let web_count = response.categories.iter().filter(|c| *c == "web").count();
        assert_eq!(web_count, 1, "Categories should be deduplicated");
    }

    #[test]
    fn test_tools_response_serialization() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080)
            .with_tool(ToolManifest::new("test_tool", "Test tool"));

        let response = server.tools_response();
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"tools\""));
        assert!(json.contains("\"total_count\""));
        assert!(json.contains("\"categories\""));
    }

    #[test]
    fn test_tools_response_tool_fields() {
        let introspection = create_test_introspection();
        let tool = ToolManifest::new("full_tool", "A fully specified tool")
            .with_category("testing")
            .with_parameter("input", "string", "Input value", true)
            .with_returns("string")
            .with_side_effects()
            .with_confirmation();

        let server = McpSelfDocServer::new(introspection, 8080).with_tool(tool);
        let response = server.tools_response();

        let full_tool = response
            .tools
            .iter()
            .find(|t| t.name == "full_tool")
            .unwrap();
        assert_eq!(full_tool.category, Some("testing".to_string()));
        assert_eq!(full_tool.parameters.len(), 1);
        assert_eq!(full_tool.returns, Some("string".to_string()));
        assert!(full_tool.has_side_effects);
        assert!(full_tool.requires_confirmation);
    }

    #[test]
    fn test_state_schema_response_with_schema() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.state_schema_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        // Test introspection has state schema
        if response.has_schema {
            assert!(response.state_type_name.is_some());
        }
    }

    #[test]
    fn test_state_schema_response_serialization() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.state_schema_response();

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"has_schema\""));
    }

    #[test]
    fn test_features_response_enhanced() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.features_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert_eq!(response.total_count, response.features.len());

        // All features should be enabled
        for feature in &response.features {
            assert!(feature.enabled);
            assert!(!feature.name.is_empty());
            assert!(!feature.description.is_empty());
        }
    }

    #[test]
    fn test_features_response_configuration() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.features_response();

        // Verify serialization includes optional configuration fields
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"features\""));
        assert!(json.contains("\"total_count\""));
        assert!(json.contains("\"enabled\""));
    }

    // ========================================================================
    // Platform Introspection Endpoint Tests
    // ========================================================================

    #[test]
    fn test_platform_version_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.platform_version_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert!(!response.version.is_empty());
        assert!(!response.rust_version.is_empty());
    }

    #[test]
    fn test_platform_version_serialization() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.platform_version_response();

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"version\""));
        assert!(json.contains("\"rust_version\""));
    }

    #[test]
    fn test_platform_features_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.platform_features_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert!(response.total_count > 0);
        assert_eq!(response.total_count, response.features.len());

        // Should have checkpointing feature
        assert!(response.features.iter().any(|f| f.name == "checkpointing"));
        // Should have streaming feature
        assert!(response.features.iter().any(|f| f.name == "streaming"));
    }

    #[test]
    fn test_platform_features_have_descriptions() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.platform_features_response();

        for feature in &response.features {
            assert!(!feature.name.is_empty());
            assert!(
                !feature.description.is_empty(),
                "Feature {} should have description",
                feature.name
            );
        }
    }

    #[test]
    fn test_platform_node_types_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.platform_node_types_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert!(response.total_count > 0);
        assert_eq!(response.total_count, response.node_types.len());

        // Should have function node type
        assert!(response.node_types.iter().any(|n| n.name == "function"));
        // Should have agent node type
        assert!(response.node_types.iter().any(|n| n.name == "agent"));
    }

    #[test]
    fn test_platform_node_types_have_examples() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.platform_node_types_response();

        // At least some node types should have examples
        let with_examples = response
            .node_types
            .iter()
            .filter(|n| n.example.is_some())
            .count();
        assert!(
            with_examples > 0,
            "At least some node types should have examples"
        );
    }

    #[test]
    fn test_platform_edge_types_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.platform_edge_types_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert!(response.total_count > 0);
        assert_eq!(response.total_count, response.edge_types.len());

        // Should have simple edge type
        assert!(response.edge_types.iter().any(|e| e.name == "simple"));
        // Should have conditional edge type
        assert!(response.edge_types.iter().any(|e| e.name == "conditional"));
    }

    #[test]
    fn test_platform_templates_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.platform_templates_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert!(response.total_count > 0);
        assert_eq!(response.total_count, response.templates.len());

        // Should have supervisor template
        assert!(response.templates.iter().any(|t| t.name == "supervisor"));
        // Should have react_agent template
        assert!(response.templates.iter().any(|t| t.name == "react_agent"));
    }

    #[test]
    fn test_platform_templates_have_use_cases() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.platform_templates_response();

        for template in &response.templates {
            assert!(
                !template.use_cases.is_empty(),
                "Template {} should have use cases",
                template.name
            );
        }
    }

    #[test]
    fn test_platform_states_response() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);
        let response = server.platform_states_response();

        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert!(response.total_count > 0);
        assert_eq!(response.total_count, response.states.len());

        // Should have JsonState
        assert!(response.states.iter().any(|s| s.name == "JsonState"));
    }

    #[test]
    fn test_platform_query_found() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        let response = server.handle_platform_query("checkpointing");
        assert!(response.available);
        assert!(response.confidence > 0.9);
        assert_eq!(response.category, Some("feature".to_string()));
    }

    #[test]
    fn test_platform_query_node_type() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        let response = server.handle_platform_query("function");
        assert!(response.available);
        assert_eq!(response.category, Some("node_type".to_string()));
    }

    #[test]
    fn test_platform_query_template() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        let response = server.handle_platform_query("supervisor");
        assert!(response.available);
        assert_eq!(response.category, Some("template".to_string()));
    }

    #[test]
    fn test_platform_query_not_found() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        let response = server.handle_platform_query("nonexistent_capability");
        assert!(!response.available);
        assert!(response.confidence < 0.5);
        assert!(response.category.is_none());
    }

    #[test]
    fn test_platform_query_case_insensitive() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        let lower = server.handle_platform_query("checkpointing");
        let upper = server.handle_platform_query("CHECKPOINTING");
        let mixed = server.handle_platform_query("Checkpointing");

        assert_eq!(lower.available, upper.available);
        assert_eq!(lower.available, mixed.available);
    }

    #[test]
    fn test_platform_responses_serialization() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        // Test all platform responses serialize correctly and contain expected fields
        let version = server.platform_version_response();
        let version_json = serde_json::to_string(&version).unwrap();
        assert!(!version_json.is_empty());
        assert!(version_json.contains("schema_version"));
        assert!(version_json.contains("\"version\":")); // DashFlow version field
        assert!(version_json.contains("rust_version"));

        let features = server.platform_features_response();
        let features_json = serde_json::to_string(&features).unwrap();
        assert!(!features_json.is_empty());
        assert!(features_json.contains("schema_version"));
        assert!(features_json.contains("features"));

        let node_types = server.platform_node_types_response();
        let node_types_json = serde_json::to_string(&node_types).unwrap();
        assert!(!node_types_json.is_empty());
        assert!(node_types_json.contains("schema_version"));
        assert!(node_types_json.contains("node_types"));

        let edge_types = server.platform_edge_types_response();
        let edge_types_json = serde_json::to_string(&edge_types).unwrap();
        assert!(!edge_types_json.is_empty());
        assert!(edge_types_json.contains("schema_version"));
        assert!(edge_types_json.contains("edge_types"));

        let templates = server.platform_templates_response();
        let templates_json = serde_json::to_string(&templates).unwrap();
        assert!(!templates_json.is_empty());
        assert!(templates_json.contains("schema_version"));
        assert!(templates_json.contains("templates"));

        let states = server.platform_states_response();
        let states_json = serde_json::to_string(&states).unwrap();
        assert!(!states_json.is_empty());
        assert!(states_json.contains("schema_version"));
        assert!(states_json.contains("\"states\":")); // States field in McpPlatformStatesResponse
    }

    // ========================================================================
    // Live Introspection Tests
    // ========================================================================

    #[test]
    fn test_live_executions_no_tracker() {
        let introspection = create_test_introspection();
        let server = McpSelfDocServer::new(introspection, 8080);

        // Without a tracker, should return empty response
        let response = server.live_executions_response();
        assert_eq!(response.schema_version, SCHEMA_VERSION);
        assert!(response.executions.is_empty());
        assert_eq!(response.active_count, 0);
        assert_eq!(response.total_count, 0);
    }

    #[test]
    fn test_live_executions_with_tracker() {
        use crate::live_introspection::ExecutionTracker;

        let introspection = create_test_introspection();
        let tracker = ExecutionTracker::new();

        // Start some executions
        let exec1 = tracker.start_execution("test_graph").unwrap();
        tracker.start_execution("test_graph2").unwrap();

        let server =
            McpSelfDocServer::new(introspection, 8080).with_execution_tracker(tracker.clone());

        let response = server.live_executions_response();
        assert_eq!(response.total_count, 2);
        assert_eq!(response.active_count, 2);
        assert_eq!(response.executions.len(), 2);

        // Verify execution summary fields
        let exec = response
            .executions
            .iter()
            .find(|e| e.execution_id == exec1)
            .unwrap();
        assert_eq!(exec.graph_name, "test_graph");
        assert_eq!(exec.status, "running");
    }

    #[test]
    fn test_live_execution_detail_not_found() {
        use crate::live_introspection::ExecutionTracker;

        let introspection = create_test_introspection();
        let tracker = ExecutionTracker::new();
        let server = McpSelfDocServer::new(introspection, 8080).with_execution_tracker(tracker);

        let response = server.live_execution_detail_response("nonexistent");
        assert!(response.is_none());
    }

    #[test]
    fn test_live_execution_detail_found() {
        use crate::live_introspection::ExecutionTracker;

        let introspection = create_test_introspection();
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("test_graph").unwrap();

        // Simulate some node execution
        tracker.enter_node(&exec_id, "process");
        tracker.exit_node_success(&exec_id, Some(serde_json::json!({"result": "ok"})));
        tracker.update_state(&exec_id, serde_json::json!({"counter": 42}));

        let server = McpSelfDocServer::new(introspection, 8080).with_execution_tracker(tracker);

        let response = server.live_execution_detail_response(&exec_id).unwrap();
        assert_eq!(response.execution_id, exec_id);
        assert_eq!(response.graph_name, "test_graph");
        assert_eq!(response.status, "running");
        assert_eq!(response.total_nodes_visited, 1);
        assert_eq!(response.state["counter"], 42);
    }

    #[test]
    fn test_live_current_node_response() {
        use crate::live_introspection::ExecutionTracker;

        let introspection = create_test_introspection();
        let tracker = ExecutionTracker::new();
        let exec_id = tracker
            .start_execution_with_entry("test_graph", "start_node")
            .unwrap();
        tracker.enter_node(&exec_id, "process_node");

        let server = McpSelfDocServer::new(introspection, 8080).with_execution_tracker(tracker);

        let response = server.live_current_node_response(&exec_id).unwrap();
        assert_eq!(response.execution_id, exec_id);
        assert_eq!(response.current_node, "process_node");
        assert_eq!(response.previous_node, Some("start_node".to_string()));
    }

    #[test]
    fn test_live_current_state_response() {
        use crate::live_introspection::ExecutionTracker;

        let introspection = create_test_introspection();
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("test_graph").unwrap();
        tracker.update_state(
            &exec_id,
            serde_json::json!({"messages": ["hello", "world"]}),
        );

        let server = McpSelfDocServer::new(introspection, 8080).with_execution_tracker(tracker);

        let response = server.live_current_state_response(&exec_id).unwrap();
        assert_eq!(response.execution_id, exec_id);
        assert_eq!(response.state["messages"][0], "hello");
        assert_eq!(response.state["messages"][1], "world");
    }

    #[test]
    fn test_live_history_response() {
        use crate::live_introspection::ExecutionTracker;

        let introspection = create_test_introspection();
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("test_graph").unwrap();

        // Execute some steps
        tracker.enter_node(&exec_id, "node1");
        tracker.exit_node_success(&exec_id, None);
        tracker.enter_node(&exec_id, "node2");
        tracker.exit_node_success(&exec_id, None);

        let server = McpSelfDocServer::new(introspection, 8080).with_execution_tracker(tracker);

        let response = server.live_history_response(&exec_id).unwrap();
        assert_eq!(response.execution_id, exec_id);
        assert_eq!(response.total_steps, 2);
        assert_eq!(response.steps.len(), 2);
        assert_eq!(response.steps[0].node_name, "node1");
        assert_eq!(response.steps[0].outcome, "success");
        assert_eq!(response.steps[1].node_name, "node2");
    }

    #[test]
    fn test_live_metrics_response() {
        use crate::live_introspection::ExecutionTracker;

        let introspection = create_test_introspection();
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("test_graph").unwrap();

        // Execute some steps to generate metrics
        tracker.enter_node(&exec_id, "node1");
        tracker.exit_node_success(&exec_id, None);
        tracker.increment_iteration(&exec_id);

        let server = McpSelfDocServer::new(introspection, 8080).with_execution_tracker(tracker);

        let response = server.live_metrics_response(&exec_id).unwrap();
        assert_eq!(response.execution_id, exec_id);
        assert_eq!(response.metrics.nodes_executed, 1);
        assert_eq!(response.metrics.nodes_succeeded, 1);
        assert_eq!(response.metrics.iteration, 1);
    }

    #[test]
    fn test_live_checkpoint_response() {
        use crate::live_introspection::ExecutionTracker;

        let introspection = create_test_introspection();
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("test_graph").unwrap();

        tracker.enable_checkpointing(&exec_id, "thread-123");
        tracker.record_checkpoint(&exec_id, 2048);

        let server = McpSelfDocServer::new(introspection, 8080).with_execution_tracker(tracker);

        let response = server.live_checkpoint_response(&exec_id).unwrap();
        assert_eq!(response.execution_id, exec_id);
        assert!(response.checkpoint.enabled);
        assert_eq!(
            response.checkpoint.thread_id,
            Some("thread-123".to_string())
        );
        assert_eq!(response.checkpoint.checkpoint_count, 1);
        assert_eq!(response.checkpoint.total_size_bytes, 2048);
    }

    #[test]
    fn test_live_responses_serialization() {
        use crate::live_introspection::ExecutionTracker;

        let introspection = create_test_introspection();
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("test_graph").unwrap();
        tracker.enter_node(&exec_id, "test");
        tracker.exit_node_success(&exec_id, None);

        let server = McpSelfDocServer::new(introspection, 8080).with_execution_tracker(tracker);

        // Test all live responses serialize correctly and contain expected fields
        let executions = server.live_executions_response();
        let executions_json = serde_json::to_string(&executions).unwrap();
        assert!(!executions_json.is_empty());
        assert!(executions_json.contains("schema_version"));
        assert!(executions_json.contains("executions"));
        assert!(executions_json.contains("total_count"));

        let detail = server.live_execution_detail_response(&exec_id).unwrap();
        let detail_json = serde_json::to_string(&detail).unwrap();
        assert!(!detail_json.is_empty());
        assert!(detail_json.contains("schema_version"));
        assert!(detail_json.contains("execution_id"));
        assert!(detail_json.contains("status"));

        let node = server.live_current_node_response(&exec_id).unwrap();
        let node_json = serde_json::to_string(&node).unwrap();
        assert!(!node_json.is_empty());
        assert!(node_json.contains("schema_version"));
        assert!(node_json.contains("execution_id"));

        let state = server.live_current_state_response(&exec_id).unwrap();
        let state_json = serde_json::to_string(&state).unwrap();
        assert!(!state_json.is_empty());
        assert!(state_json.contains("schema_version"));
        assert!(state_json.contains("execution_id"));

        let history = server.live_history_response(&exec_id).unwrap();
        let history_json = serde_json::to_string(&history).unwrap();
        assert!(!history_json.is_empty());
        assert!(history_json.contains("schema_version"));
        assert!(history_json.contains("execution_id"));
        assert!(history_json.contains("\"steps\":")); // Steps field in McpLiveHistoryResponse

        let metrics = server.live_metrics_response(&exec_id).unwrap();
        let metrics_json = serde_json::to_string(&metrics).unwrap();
        assert!(!metrics_json.is_empty());
        assert!(metrics_json.contains("schema_version"));
        assert!(metrics_json.contains("execution_id"));

        let checkpoint = server.live_checkpoint_response(&exec_id).unwrap();
        let checkpoint_json = serde_json::to_string(&checkpoint).unwrap();
        assert!(!checkpoint_json.is_empty());
        assert!(checkpoint_json.contains("schema_version"));
        assert!(checkpoint_json.contains("execution_id"));
    }

    #[test]
    fn test_with_execution_tracker_builder() {
        use crate::live_introspection::ExecutionTracker;

        let introspection = create_test_introspection();
        let tracker = ExecutionTracker::new();

        let server =
            McpSelfDocServer::new(introspection, 8080).with_execution_tracker(tracker.clone());

        assert!(server.execution_tracker().is_some());
    }

    #[test]
    fn test_live_execution_completed_status() {
        use crate::live_introspection::ExecutionTracker;

        let introspection = create_test_introspection();
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("test_graph").unwrap();
        tracker.complete_execution(&exec_id);

        let server = McpSelfDocServer::new(introspection, 8080).with_execution_tracker(tracker);

        let response = server.live_execution_detail_response(&exec_id).unwrap();
        assert_eq!(response.status, "completed");
    }

    #[test]
    fn test_live_execution_failed_status() {
        use crate::live_introspection::ExecutionTracker;

        let introspection = create_test_introspection();
        let tracker = ExecutionTracker::new();
        let exec_id = tracker.start_execution("test_graph").unwrap();
        tracker.fail_execution(&exec_id, "Something went wrong");

        let server = McpSelfDocServer::new(introspection, 8080).with_execution_tracker(tracker);

        let response = server.live_execution_detail_response(&exec_id).unwrap();
        assert_eq!(response.status, "failed");
        assert_eq!(response.error, Some("Something went wrong".to_string()));
    }
}

// ============================================================================
// UnifiedMcpServer - Combines Module Discovery + Graph Introspection
// ============================================================================

/// Unified MCP server that can operate in two modes:
/// - Discovery-only: module endpoints + platform info (CLI default)
/// - Full: all endpoints including graph introspection
///
/// This allows the CLI to serve module discovery without requiring a compiled
/// graph, while still providing the full MCP API when a graph is available.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::mcp_self_doc::UnifiedMcpServer;
/// use std::path::Path;
///
/// // Discovery-only mode (CLI default)
/// let server = UnifiedMcpServer::discovery_only(Path::new("crates/dashflow/src"));
/// server.serve(3200).await?;
///
/// // Full mode with graph introspection
/// let server = UnifiedMcpServer::with_graph(introspection);
/// server.serve(3200).await?;
/// ```
#[cfg(feature = "mcp-server")]
#[derive(Clone)]
pub struct UnifiedMcpServer {
    /// Discovered modules (always available)
    modules: Arc<Vec<dashflow_module_discovery::ModuleInfo>>,
    /// Graph introspection (only available in full mode)
    graph_server: Option<McpSelfDocServer>,
}

#[cfg(feature = "mcp-server")]
impl UnifiedMcpServer {
    /// Create server for discovery-only mode (CLI default).
    ///
    /// This mode serves module discovery endpoints without requiring a compiled graph.
    /// Graph-related endpoints (`/mcp/*`) will return 503 Service Unavailable.
    pub fn discovery_only(src_path: impl AsRef<std::path::Path>) -> Self {
        let modules = dashflow_module_discovery::discover_modules(src_path);
        Self {
            modules: Arc::new(modules),
            graph_server: None,
        }
    }

    /// Create server for discovery-only mode using workspace-wide module discovery.
    ///
    /// This scans multiple crates in the workspace for a more complete module listing.
    pub fn discovery_only_workspace(workspace_root: impl AsRef<std::path::Path>) -> Self {
        let crates = dashflow_module_discovery::default_workspace_crates();
        let modules =
            dashflow_module_discovery::discover_workspace_modules(workspace_root, &crates);
        Self {
            modules: Arc::new(modules),
            graph_server: None,
        }
    }

    /// Create server with full graph introspection.
    ///
    /// This mode serves all endpoints including graph-specific ones.
    pub fn with_graph(introspection: crate::executor::GraphIntrospection, port: u16) -> Self {
        // Get modules from platform introspection
        let modules = dashflow_module_discovery::discover_modules("crates/dashflow/src");
        Self {
            modules: Arc::new(modules),
            graph_server: Some(McpSelfDocServer::new(introspection, port)),
        }
    }

    /// Check if graph introspection is available.
    #[must_use]
    pub fn has_graph(&self) -> bool {
        self.graph_server.is_some()
    }

    /// Get the number of discovered modules.
    #[must_use]
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// List all discovered modules.
    #[must_use]
    pub fn list_modules(&self) -> &[dashflow_module_discovery::ModuleInfo] {
        &self.modules
    }

    /// Get a specific module by name.
    #[must_use]
    pub fn get_module(&self, name: &str) -> Option<&dashflow_module_discovery::ModuleInfo> {
        self.modules
            .iter()
            .find(|m| m.name == name || m.path == name)
    }

    /// Search modules by query string.
    #[must_use]
    pub fn search_modules(&self, query: &str) -> Vec<&dashflow_module_discovery::ModuleInfo> {
        let query_lower = query.to_lowercase();
        self.modules
            .iter()
            .filter(|m| {
                m.name.to_lowercase().contains(&query_lower)
                    || m.path.to_lowercase().contains(&query_lower)
                    || m.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// Start the unified MCP server.
    ///
    /// # Errors
    ///
    /// Returns error if the server fails to start.
    pub async fn serve(self, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use axum::{routing::get, Router};

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

        let app = Router::new()
            // Module discovery endpoints (always available)
            .route("/modules", get(unified_handle_modules))
            .route("/modules/:name", get(unified_handle_module_detail))
            .route("/search", get(unified_handle_search))
            .route("/health", get(unified_handle_health))
            // Graph introspection endpoints (return 503 if no graph)
            .route("/mcp/about", get(unified_handle_mcp_about))
            .route("/mcp/capabilities", get(unified_handle_mcp_capabilities))
            .route("/mcp/architecture", get(unified_handle_mcp_architecture))
            .route(
                "/mcp/implementation",
                get(unified_handle_mcp_implementation),
            )
            .route("/mcp/nodes", get(unified_handle_mcp_nodes))
            .route("/mcp/nodes/:name", get(unified_handle_mcp_node_detail))
            .route("/mcp/edges", get(unified_handle_mcp_edges))
            .route("/mcp/features", get(unified_handle_mcp_features))
            .route("/mcp/tools", get(unified_handle_mcp_tools))
            // Optimizer selection endpoints
            .route(
                "/introspect/optimize",
                axum::routing::post(unified_handle_optimize_select),
            )
            .route(
                "/introspect/optimize/history",
                get(unified_handle_optimize_history),
            )
            .with_state(self);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

/// Response for listing modules (UnifiedMcpServer)
#[cfg(feature = "mcp-server")]
#[derive(Serialize)]
pub struct UnifiedModuleListResponse {
    pub count: usize,
    pub modules: Vec<dashflow_module_discovery::ModuleInfo>,
}

/// Response for single module (UnifiedMcpServer)
#[cfg(feature = "mcp-server")]
#[derive(Serialize)]
pub struct UnifiedModuleResponse {
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<dashflow_module_discovery::ModuleInfo>,
}

/// Response for search (UnifiedMcpServer)
#[cfg(feature = "mcp-server")]
#[derive(Serialize)]
pub struct UnifiedSearchResponse {
    pub query: String,
    pub count: usize,
    pub results: Vec<dashflow_module_discovery::ModuleInfo>,
}

/// Health check response
#[cfg(feature = "mcp-server")]
#[derive(Serialize)]
pub struct UnifiedHealthResponse {
    pub status: String,
    pub module_count: usize,
    pub graph_available: bool,
}

// Handlers for UnifiedMcpServer

#[cfg(feature = "mcp-server")]
async fn unified_handle_modules(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
) -> axum::Json<UnifiedModuleListResponse> {
    let modules = server.modules.as_ref().clone();
    axum::Json(UnifiedModuleListResponse {
        count: modules.len(),
        modules,
    })
}

#[cfg(feature = "mcp-server")]
async fn unified_handle_module_detail(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> axum::Json<UnifiedModuleResponse> {
    let module = server.get_module(&name).cloned();
    axum::Json(UnifiedModuleResponse {
        found: module.is_some(),
        module,
    })
}

#[cfg(feature = "mcp-server")]
async fn unified_handle_search(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> axum::Json<UnifiedSearchResponse> {
    let query = params.get("q").map(|s| s.as_str()).unwrap_or("");
    let results: Vec<dashflow_module_discovery::ModuleInfo> =
        server.search_modules(query).into_iter().cloned().collect();
    axum::Json(UnifiedSearchResponse {
        query: query.to_string(),
        count: results.len(),
        results,
    })
}

#[cfg(feature = "mcp-server")]
async fn unified_handle_health(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
) -> axum::Json<UnifiedHealthResponse> {
    axum::Json(UnifiedHealthResponse {
        status: "ok".to_string(),
        module_count: server.module_count(),
        graph_available: server.has_graph(),
    })
}

/// Error response for when graph introspection is not available
#[cfg(feature = "mcp-server")]
fn graph_required_response() -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    (
        StatusCode::SERVICE_UNAVAILABLE,
        axum::Json(serde_json::json!({
            "error": "graph_required",
            "message": "This endpoint requires a compiled graph. Start server with --with-graph flag.",
            "available_endpoints": ["/modules", "/modules/:name", "/search", "/health"]
        })),
    ).into_response()
}

#[cfg(feature = "mcp-server")]
async fn unified_handle_mcp_about(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    match &server.graph_server {
        Some(gs) => axum::Json(gs.about_response()).into_response(),
        None => graph_required_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn unified_handle_mcp_capabilities(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    match &server.graph_server {
        Some(gs) => axum::Json(gs.capabilities_response()).into_response(),
        None => graph_required_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn unified_handle_mcp_architecture(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    match &server.graph_server {
        Some(gs) => axum::Json(gs.architecture_response()).into_response(),
        None => graph_required_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn unified_handle_mcp_implementation(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    match &server.graph_server {
        Some(gs) => axum::Json(gs.implementation_response()).into_response(),
        None => graph_required_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn unified_handle_mcp_nodes(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    match &server.graph_server {
        Some(gs) => axum::Json(gs.nodes_list_response()).into_response(),
        None => graph_required_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn unified_handle_mcp_node_detail(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    match &server.graph_server {
        Some(gs) => match gs.node_detail_response(&name) {
            Some(detail) => axum::Json(detail).into_response(),
            None => (
                StatusCode::NOT_FOUND,
                axum::Json(serde_json::json!({
                    "error": "Node not found",
                    "node_name": name
                })),
            )
                .into_response(),
        },
        None => graph_required_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn unified_handle_mcp_edges(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    match &server.graph_server {
        Some(gs) => axum::Json(gs.edges_response()).into_response(),
        None => graph_required_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn unified_handle_mcp_features(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    match &server.graph_server {
        Some(gs) => axum::Json(gs.features_response()).into_response(),
        None => graph_required_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn unified_handle_mcp_tools(
    axum::extract::State(server): axum::extract::State<UnifiedMcpServer>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    match &server.graph_server {
        Some(gs) => axum::Json(gs.tools_response()).into_response(),
        None => graph_required_response(),
    }
}

// ============================================================================
// Optimizer Selection MCP Endpoints
// ============================================================================

/// Request for optimizer selection via MCP.
///
/// External AI agents can query this endpoint to get optimizer recommendations.
#[cfg(feature = "mcp-server")]
#[derive(Debug, Clone, Deserialize)]
pub struct McpOptimizeSelectRequest {
    /// Number of training examples available (0 = unknown)
    #[serde(default)]
    pub num_examples: usize,

    /// Task type hint (e.g., "classification", "generation", "qa")
    #[serde(default)]
    pub task_type: Option<String>,

    /// Compute budget (e.g., "low", "medium", "high")
    #[serde(default)]
    pub budget: Option<String>,

    /// Whether fine-tuning is available
    #[serde(default)]
    pub can_finetune: bool,

    /// Optimizers to exclude from selection
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Response for optimizer selection via MCP.
#[cfg(feature = "mcp-server")]
#[derive(Debug, Clone, Serialize)]
pub struct McpOptimizeSelectResponse {
    /// Schema version for response stability
    pub schema_version: String,

    /// Recommended optimizer name
    pub optimizer: String,

    /// Confidence score (0.0-1.0)
    pub confidence: f64,

    /// Human-readable explanation
    pub explanation: String,

    /// Alternative optimizers with reasons
    pub alternatives: Vec<McpOptimizerAlternative>,
}

/// Alternative optimizer with explanation.
#[cfg(feature = "mcp-server")]
#[derive(Debug, Clone, Serialize)]
pub struct McpOptimizerAlternative {
    /// Optimizer name
    pub name: String,

    /// Why this is an alternative (not first choice)
    pub reason: String,
}

/// Response for optimizer history via MCP.
#[cfg(feature = "mcp-server")]
#[derive(Debug, Clone, Serialize)]
pub struct McpOptimizeHistoryResponse {
    /// Schema version for response stability
    pub schema_version: String,

    /// Number of recorded outcomes
    pub total_outcomes: usize,

    /// Outcomes grouped by optimizer
    pub by_optimizer: Vec<McpOptimizerStats>,
}

/// Stats for a specific optimizer.
#[cfg(feature = "mcp-server")]
#[derive(Debug, Clone, Serialize)]
pub struct McpOptimizerStats {
    /// Optimizer name
    pub name: String,

    /// Number of times used
    pub uses: usize,

    /// Average improvement score
    pub avg_improvement: f64,

    /// Success rate (outcomes with improvement > 0)
    pub success_rate: f64,
}

/// Handler for optimizer selection endpoint.
#[cfg(feature = "mcp-server")]
async fn unified_handle_optimize_select(
    axum::Json(request): axum::Json<McpOptimizeSelectRequest>,
) -> axum::Json<McpOptimizeSelectResponse> {
    use crate::optimize::auto_optimizer::{
        AutoOptimizer, ComputeBudget, OptimizationContext, TaskType,
    };

    // Parse task type from request, defaulting to Generic
    let task_type = request
        .task_type
        .as_deref()
        .map(|t| match t.to_lowercase().as_str() {
            "classification" => TaskType::Classification,
            "qa" | "question_answering" => TaskType::QuestionAnswering,
            "summarization" => TaskType::Summarization,
            "code" | "code_generation" => TaskType::CodeGeneration,
            "math" | "math_reasoning" => TaskType::MathReasoning,
            "agent" => TaskType::Agent,
            "reasoning" => TaskType::Reasoning,
            _ => TaskType::Generic,
        })
        .unwrap_or(TaskType::Generic);

    // Parse compute budget
    let compute_budget = request
        .budget
        .as_deref()
        .map(|b| match b.to_lowercase().as_str() {
            "minimal" => ComputeBudget::Minimal,
            "low" => ComputeBudget::Low,
            "medium" => ComputeBudget::Medium,
            "high" => ComputeBudget::High,
            "unlimited" => ComputeBudget::Unlimited,
            _ => ComputeBudget::Medium,
        })
        .unwrap_or(ComputeBudget::Medium);

    let context = OptimizationContext {
        num_examples: request.num_examples,
        task_type,
        compute_budget,
        can_finetune: request.can_finetune,
        excluded_optimizers: request.exclude.clone(),
        has_embedding_model: false,
        available_capabilities: vec!["metric_function".to_string()],
        preferred_tier: None,
    };

    // Select optimizer using static method
    let result = AutoOptimizer::select(&context);

    // Build alternatives list
    let alternatives: Vec<McpOptimizerAlternative> = result
        .alternatives
        .iter()
        .map(|alt| McpOptimizerAlternative {
            name: alt.name.clone(),
            reason: alt.reason.clone(),
        })
        .collect();

    axum::Json(McpOptimizeSelectResponse {
        schema_version: SCHEMA_VERSION.to_string(),
        optimizer: result.optimizer_name.clone(),
        confidence: result.confidence,
        explanation: result.reason.clone(),
        alternatives,
    })
}

/// Handler for optimizer history endpoint.
#[cfg(feature = "mcp-server")]
async fn unified_handle_optimize_history(
    _server: axum::extract::State<UnifiedMcpServer>,
) -> axum::Json<McpOptimizeHistoryResponse> {
    use crate::optimize::auto_optimizer::AutoOptimizer;

    let optimizer = AutoOptimizer::new();

    // Try to load outcomes (may fail if storage not initialized)
    let outcomes = optimizer.load_outcomes().await.unwrap_or_default();

    // Group by optimizer
    let mut by_optimizer: std::collections::HashMap<String, Vec<f64>> =
        std::collections::HashMap::new();
    for outcome in &outcomes {
        by_optimizer
            .entry(outcome.optimizer_name.clone())
            .or_default()
            .push(outcome.improvement);
    }

    let stats: Vec<McpOptimizerStats> = by_optimizer
        .into_iter()
        .map(|(name, improvements)| {
            let uses = improvements.len();
            let avg_improvement = improvements.iter().sum::<f64>() / uses as f64;
            let success_rate =
                improvements.iter().filter(|&&i| i > 0.0).count() as f64 / uses as f64;
            McpOptimizerStats {
                name,
                uses,
                avg_improvement,
                success_rate,
            }
        })
        .collect();

    axum::Json(McpOptimizeHistoryResponse {
        schema_version: SCHEMA_VERSION.to_string(),
        total_outcomes: outcomes.len(),
        by_optimizer: stats,
    })
}
