// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Help Generation for DashFlow Applications
//!
//! This module provides CLI help generation at multiple levels of detail:
//! - `HelpLevel::Brief` - Quick overview (--help)
//! - `HelpLevel::More` - Architecture details (--help-more)
//! - `HelpLevel::Implementation` - Full internals (--help-implementation)

use crate::executor::GraphIntrospection;
use crate::introspection::{CapabilityManifest, GraphManifest};
use crate::platform_registry::{AppArchitecture, PlatformRegistry};

/// Level of detail for help output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HelpLevel {
    /// tl;dr - Quick overview (30 seconds to read)
    #[default]
    Brief,
    /// ok-a-little-more - Architecture and structure (2-3 minutes)
    More,
    /// detailed - Full implementation details (10-15 minutes)
    Implementation,
}

impl HelpLevel {
    /// Parse from command-line argument.
    #[must_use]
    pub fn from_arg(arg: &str) -> Option<Self> {
        match arg {
            "--help" | "-h" => Some(Self::Brief),
            "--help-more" => Some(Self::More),
            "--help-implementation" | "--help-impl" => Some(Self::Implementation),
            _ => None,
        }
    }

    /// Find help level from command-line arguments.
    ///
    /// Scans through all arguments and returns the first help flag found.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow::mcp_self_doc::HelpLevel;
    ///
    /// let args: Vec<String> = vec!["myapp".into(), "--help".into()];
    /// let level = HelpLevel::from_args(&args);
    /// assert_eq!(level, Some(HelpLevel::Brief));
    /// ```
    #[must_use]
    pub fn from_args<I, S>(args: I) -> Option<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        args.into_iter()
            .find_map(|arg| Self::from_arg(arg.as_ref()))
    }

    /// Check if the given arguments contain any help flag.
    ///
    /// Convenience method for quickly checking if help was requested.
    #[must_use]
    pub fn is_help_requested<I, S>(args: I) -> bool
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::from_args(args).is_some()
    }
}

/// Help generator for DashFlow applications.
///
/// Produces auto-generated help text at different levels of detail.
#[derive(Debug, Clone)]
pub struct HelpGenerator {
    manifest: GraphManifest,
    platform: PlatformRegistry,
    architecture: AppArchitecture,
    capabilities: CapabilityManifest,
    /// Application name override (defaults to graph name)
    app_name: Option<String>,
    /// Application version override
    app_version: Option<String>,
    /// Application description override
    app_description: Option<String>,
}

impl HelpGenerator {
    /// Create a new help generator from graph introspection.
    #[must_use]
    pub fn new(introspection: GraphIntrospection) -> Self {
        Self {
            manifest: introspection.manifest,
            platform: introspection.platform,
            architecture: introspection.architecture,
            capabilities: introspection.capabilities,
            app_name: None,
            app_version: None,
            app_description: None,
        }
    }

    /// Set custom application name.
    #[must_use]
    pub fn with_app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = Some(name.into());
        self
    }

    /// Set custom application version.
    #[must_use]
    pub fn with_app_version(mut self, version: impl Into<String>) -> Self {
        self.app_version = Some(version.into());
        self
    }

    /// Set custom application description.
    #[must_use]
    pub fn with_app_description(mut self, description: impl Into<String>) -> Self {
        self.app_description = Some(description.into());
        self
    }

    /// Generate help text at the specified level.
    #[must_use]
    pub fn generate(&self, level: HelpLevel) -> String {
        match level {
            HelpLevel::Brief => self.help_brief(),
            HelpLevel::More => self.help_more(),
            HelpLevel::Implementation => self.help_implementation(),
        }
    }

    /// Get application name.
    fn app_name(&self) -> &str {
        self.app_name
            .as_deref()
            .or(self.manifest.graph_name.as_deref())
            .unwrap_or("DashFlow Application")
    }

    /// Get application version.
    fn app_version(&self) -> &str {
        self.app_version.as_deref().unwrap_or("1.0.0")
    }

    /// Get application description.
    fn app_description(&self) -> String {
        self.app_description
            .clone()
            .or_else(|| {
                self.manifest
                    .metadata
                    .custom
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| "A DashFlow-powered AI agent".to_string())
    }

    /// Generate brief help (tl;dr).
    fn help_brief(&self) -> String {
        let capabilities = self.list_capabilities_brief();

        format!(
            "{} v{}\n\n\
             {}\n\n\
             CAPABILITIES:\n\
             {}\n\n\
             USAGE:\n\
             {} [OPTIONS]\n\n\
             OPTIONS:\n\
             --help              Show this help message\n\
             --help-more         Show detailed architecture\n\
             --help-implementation  Show code locations and internals",
            self.app_name(),
            self.app_version(),
            self.app_description(),
            capabilities,
            self.app_name().to_lowercase().replace(' ', "_")
        )
    }

    /// Generate detailed help (ok-a-little-more).
    fn help_more(&self) -> String {
        let node_list = self.list_nodes_with_descriptions();
        let features_used = self.list_dashflow_features_used();
        let execution_flow = self.explain_execution_flow();

        format!(
            "{}\n\n\
             ARCHITECTURE:\n\
             This app is built with DashFlow and consists of:\n\
             - {} nodes (processing units)\n\
             - {} edges (connections)\n\
             - {} tools available\n\n\
             NODES:\n\
             {}\n\n\
             DASHFLOW FEATURES USED:\n\
             {}\n\n\
             EXECUTION FLOW:\n\
             {}\n\n\
             Use --help-implementation for code locations and versions",
            self.help_brief(),
            self.manifest.node_count(),
            self.manifest.edge_count(),
            self.capabilities.tools.len(),
            node_list,
            features_used,
            execution_flow
        )
    }

    /// Generate implementation help (full details).
    fn help_implementation(&self) -> String {
        let node_versions = self.list_node_versions();
        let dependencies = self.list_dependencies();
        let internal_apis = self.list_internal_apis();

        format!(
            "{}\n\n\
             IMPLEMENTATION DETAILS:\n\n\
             NODE VERSIONS:\n\
             {}\n\n\
             DASHFLOW VERSION:\n\
             - dashflow: {}\n\n\
             INTERNAL APIS:\n\
             {}\n\n\
             DEPENDENCIES (DashFlow crates):\n\
             {}",
            self.help_more(),
            node_versions,
            self.platform.version,
            internal_apis,
            dependencies
        )
    }

    /// List capabilities briefly.
    fn list_capabilities_brief(&self) -> String {
        if self.capabilities.tools.is_empty() {
            return "- Graph-based AI orchestration".to_string();
        }

        self.capabilities
            .tools
            .iter()
            .take(10) // Limit for brief view
            .map(|t| format!("- {}: {}", t.name, t.description))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// List nodes with descriptions.
    fn list_nodes_with_descriptions(&self) -> String {
        self.manifest
            .nodes
            .iter()
            .map(|(name, node)| {
                let desc = node
                    .description
                    .as_deref()
                    .unwrap_or("No description available");
                let node_type = format!("{:?}", node.node_type).to_lowercase();
                format!("- {} ({}): {}", name, node_type, desc)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// List DashFlow features used.
    fn list_dashflow_features_used(&self) -> String {
        if self.architecture.dashflow_features_used.is_empty() {
            return "- StateGraph (core orchestration)".to_string();
        }

        self.architecture
            .dashflow_features_used
            .iter()
            .map(|f| format!("- {} ({})", f.name, f.description))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Explain execution flow.
    fn explain_execution_flow(&self) -> String {
        let entry = &self.manifest.entry_point;
        let terminals = self.manifest.terminal_nodes();
        let decision_points = self.manifest.decision_points();

        let mut flow = format!("1. Execution starts at node '{}'\n", entry);

        if !decision_points.is_empty() {
            flow.push_str(&format!(
                "2. Decision points: {}\n",
                decision_points.join(", ")
            ));
        }

        if !terminals.is_empty() {
            flow.push_str(&format!("3. Execution ends at: {}", terminals.join(" or ")));
        }

        flow
    }

    /// List node versions.
    fn list_node_versions(&self) -> String {
        self.manifest
            .nodes
            .iter()
            .map(|(name, node)| {
                let version = node
                    .metadata
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("1.0.0");
                format!("- {}: v{}", name, version)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// List dependencies.
    fn list_dependencies(&self) -> String {
        self.architecture
            .dependencies
            .iter()
            .filter(|d| d.is_dashflow)
            .map(|d| {
                let version = d.version.as_deref().unwrap_or("unknown");
                format!("- {}: {} ({})", d.name, version, d.purpose)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// List internal APIs.
    fn list_internal_apis(&self) -> String {
        [
            "- StateGraph: dashflow::StateGraph",
            "- CompiledGraph: dashflow::CompiledGraph",
            "- Introspection: dashflow::introspection",
            "- Platform: dashflow::platform_registry",
        ]
        .join("\n")
    }
}

// ============================================================================
// CLI Integration Helpers
// ============================================================================

/// Configuration for CLI help behavior.
///
/// Customizes how help is displayed when triggered from command-line arguments.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::mcp_self_doc::CliHelpConfig;
///
/// let config = CliHelpConfig::new()
///     .with_app_name("My AI Agent")
///     .with_app_version("2.0.0")
///     .with_app_description("An intelligent assistant");
/// ```
#[derive(Debug, Clone, Default)]
pub struct CliHelpConfig {
    /// Override application name (defaults to graph name)
    pub app_name: Option<String>,
    /// Override application version
    pub app_version: Option<String>,
    /// Override application description
    pub app_description: Option<String>,
    /// Custom output writer (defaults to stdout)
    pub output_to_stderr: bool,
}

impl CliHelpConfig {
    /// Create a new CLI help configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set custom application name.
    #[must_use]
    pub fn with_app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = Some(name.into());
        self
    }

    /// Set custom application version.
    #[must_use]
    pub fn with_app_version(mut self, version: impl Into<String>) -> Self {
        self.app_version = Some(version.into());
        self
    }

    /// Set custom application description.
    #[must_use]
    pub fn with_app_description(mut self, description: impl Into<String>) -> Self {
        self.app_description = Some(description.into());
        self
    }

    /// Output help to stderr instead of stdout.
    #[must_use]
    pub fn output_to_stderr(mut self) -> Self {
        self.output_to_stderr = true;
        self
    }
}

/// Result of processing CLI help arguments.
///
/// Indicates whether help was requested and displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliHelpResult {
    /// No help flag was found in arguments, continue normal execution
    Continue,
    /// Help was displayed for the specified level
    Displayed(HelpLevel),
}

impl CliHelpResult {
    /// Returns `true` if help was displayed (program should exit).
    #[must_use]
    pub fn should_exit(&self) -> bool {
        matches!(self, Self::Displayed(_))
    }

    /// Returns `true` if normal execution should continue.
    #[must_use]
    pub fn should_continue(&self) -> bool {
        matches!(self, Self::Continue)
    }
}

/// Process command-line arguments for help flags.
///
/// This is a convenience function for DashFlow applications to handle
/// `--help`, `--help-more`, and `--help-implementation` flags.
///
/// # Arguments
///
/// * `args` - Command-line arguments (typically from `std::env::args()`)
/// * `introspection` - Graph introspection data
/// * `config` - Optional configuration for customizing output
///
/// # Returns
///
/// * `CliHelpResult::Continue` - No help flag found, continue execution
/// * `CliHelpResult::Displayed(level)` - Help was printed, exit gracefully
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::mcp_self_doc::{process_cli_help, CliHelpConfig, CliHelpResult};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let graph = build_my_graph()?;
///     let compiled = graph.compile()?;
///
///     // Handle help flags before normal execution
///     let result = process_cli_help(
///         std::env::args(),
///         compiled.introspect(),
///         None,
///     );
///
///     if result.should_exit() {
///         return Ok(());
///     }
///
///     // Normal execution continues...
///     compiled.invoke(initial_state).await?;
///     Ok(())
/// }
/// ```
pub fn process_cli_help<I, S>(
    args: I,
    introspection: GraphIntrospection,
    config: Option<CliHelpConfig>,
) -> CliHelpResult
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let Some(level) = HelpLevel::from_args(args) else {
        return CliHelpResult::Continue;
    };

    let config = config.unwrap_or_default();

    let mut generator = HelpGenerator::new(introspection);

    if let Some(ref name) = config.app_name {
        generator = generator.with_app_name(name);
    }
    if let Some(ref version) = config.app_version {
        generator = generator.with_app_version(version);
    }
    if let Some(ref description) = config.app_description {
        generator = generator.with_app_description(description);
    }

    let help_text = generator.generate(level);

    if config.output_to_stderr {
        eprintln!("{help_text}");
    } else {
        println!("{help_text}");
    }

    CliHelpResult::Displayed(level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{
        graph_manifest::{EdgeManifest, GraphMetadata, NodeManifest, NodeType},
        CapabilityManifest, GraphManifest, ToolManifest,
    };
    use crate::platform_registry::{
        AppArchitecture, ArchitectureGraphInfo, ArchitectureMetadata, Dependency, FeatureUsage,
        PlatformMetadata, PlatformRegistry,
    };
    use std::collections::HashMap;

    // ==========================================================================
    // Test Helper Functions
    // ==========================================================================

    fn create_test_graph_manifest() -> GraphManifest {
        let mut nodes = HashMap::new();
        nodes.insert(
            "start".to_string(),
            NodeManifest {
                name: "start".to_string(),
                description: Some("Entry point node".to_string()),
                node_type: NodeType::Function,
                tools_available: vec![],
                metadata: HashMap::new(),
            },
        );
        nodes.insert(
            "process".to_string(),
            NodeManifest {
                name: "process".to_string(),
                description: Some("Processing node".to_string()),
                node_type: NodeType::Agent,
                tools_available: vec!["search".to_string()],
                metadata: HashMap::new(),
            },
        );
        nodes.insert(
            "decision".to_string(),
            NodeManifest {
                name: "decision".to_string(),
                description: None,
                node_type: NodeType::Function,
                tools_available: vec![],
                metadata: HashMap::new(),
            },
        );

        let mut edges = HashMap::new();
        edges.insert(
            "start".to_string(),
            vec![EdgeManifest {
                from: "start".to_string(),
                to: "process".to_string(),
                is_conditional: false,
                is_parallel: false,
                condition_label: None,
                description: None,
            }],
        );
        edges.insert(
            "process".to_string(),
            vec![EdgeManifest {
                from: "process".to_string(),
                to: "decision".to_string(),
                is_conditional: false,
                is_parallel: false,
                condition_label: None,
                description: None,
            }],
        );
        edges.insert(
            "decision".to_string(),
            vec![
                EdgeManifest {
                    from: "decision".to_string(),
                    to: "process".to_string(),
                    is_conditional: true,
                    is_parallel: false,
                    condition_label: Some("retry".to_string()),
                    description: None,
                },
                EdgeManifest {
                    from: "decision".to_string(),
                    to: "__end__".to_string(),
                    is_conditional: true,
                    is_parallel: false,
                    condition_label: Some("done".to_string()),
                    description: None,
                },
            ],
        );

        let mut custom_metadata = HashMap::new();
        custom_metadata.insert(
            "description".to_string(),
            serde_json::json!("A test AI agent"),
        );

        GraphManifest {
            graph_id: Some("test-graph-123".to_string()),
            graph_name: Some("TestGraph".to_string()),
            entry_point: "start".to_string(),
            nodes,
            edges,
            state_schema: None,
            metadata: GraphMetadata {
                version: Some("1.0.0".to_string()),
                author: Some("Test Author".to_string()),
                created_at: None,
                has_cycles: true,
                has_parallel_edges: false,
                custom: custom_metadata,
            },
            node_configs: HashMap::new(),
        }
    }

    fn create_test_platform_registry() -> PlatformRegistry {
        PlatformRegistry {
            version: "1.11.0".to_string(),
            modules: vec![],
            features: vec![],
            crates: vec![],
            metadata: PlatformMetadata {
                name: "DashFlow".to_string(),
                repository: Some("https://github.com/test/dashflow".to_string()),
                documentation: None,
                license: Some("MIT".to_string()),
            },
        }
    }

    fn create_test_app_architecture() -> AppArchitecture {
        AppArchitecture {
            graph_structure: ArchitectureGraphInfo {
                name: Some("TestGraph".to_string()),
                entry_point: "start".to_string(),
                node_count: 3,
                edge_count: 4,
                node_names: vec![
                    "start".to_string(),
                    "process".to_string(),
                    "decision".to_string(),
                ],
                has_cycles: true,
                has_conditional_edges: true,
                has_parallel_edges: false,
            },
            dashflow_features_used: vec![
                FeatureUsage {
                    name: "StateGraph".to_string(),
                    category: "core".to_string(),
                    description: "Core graph orchestration".to_string(),
                    apis_used: vec!["StateGraph::new".to_string()],
                    is_core: true,
                },
                FeatureUsage {
                    name: "Checkpoint".to_string(),
                    category: "persistence".to_string(),
                    description: "State persistence".to_string(),
                    apis_used: vec!["MemoryCheckpoint::new".to_string()],
                    is_core: false,
                },
            ],
            custom_code: vec![],
            dependencies: vec![
                Dependency {
                    name: "dashflow".to_string(),
                    version: Some("1.11.0".to_string()),
                    purpose: "Core framework".to_string(),
                    is_dashflow: true,
                    apis_used: vec![],
                },
                Dependency {
                    name: "tokio".to_string(),
                    version: Some("1.0.0".to_string()),
                    purpose: "Async runtime".to_string(),
                    is_dashflow: false,
                    apis_used: vec![],
                },
            ],
            metadata: ArchitectureMetadata {
                dashflow_version: "1.11.0".to_string(),
                analyzed_at: None,
                notes: vec![],
            },
        }
    }

    fn create_test_capability_manifest() -> CapabilityManifest {
        CapabilityManifest {
            tools: vec![
                ToolManifest {
                    name: "search".to_string(),
                    description: "Search the web".to_string(),
                    category: Some("web".to_string()),
                    parameters: vec![],
                    returns: Some("String".to_string()),
                    has_side_effects: false,
                    requires_confirmation: false,
                    metadata: HashMap::new(),
                },
                ToolManifest {
                    name: "write_file".to_string(),
                    description: "Write to a file".to_string(),
                    category: Some("filesystem".to_string()),
                    parameters: vec![],
                    returns: None,
                    has_side_effects: true,
                    requires_confirmation: true,
                    metadata: HashMap::new(),
                },
            ],
            models: vec![],
            storage: vec![],
            custom: HashMap::new(),
        }
    }

    fn create_test_introspection() -> GraphIntrospection {
        GraphIntrospection {
            manifest: create_test_graph_manifest(),
            platform: create_test_platform_registry(),
            architecture: create_test_app_architecture(),
            capabilities: create_test_capability_manifest(),
        }
    }

    fn create_minimal_introspection() -> GraphIntrospection {
        GraphIntrospection {
            manifest: GraphManifest {
                graph_id: None,
                graph_name: None,
                entry_point: "main".to_string(),
                nodes: HashMap::new(),
                edges: HashMap::new(),
                state_schema: None,
                metadata: GraphMetadata {
                    version: None,
                    author: None,
                    created_at: None,
                    has_cycles: false,
                    has_parallel_edges: false,
                    custom: HashMap::new(),
                },
                node_configs: HashMap::new(),
            },
            platform: PlatformRegistry {
                version: "1.0.0".to_string(),
                modules: vec![],
                features: vec![],
                crates: vec![],
                metadata: PlatformMetadata::default(),
            },
            architecture: AppArchitecture {
                graph_structure: ArchitectureGraphInfo {
                    name: None,
                    entry_point: "main".to_string(),
                    node_count: 0,
                    edge_count: 0,
                    node_names: vec![],
                    has_cycles: false,
                    has_conditional_edges: false,
                    has_parallel_edges: false,
                },
                dashflow_features_used: vec![],
                custom_code: vec![],
                dependencies: vec![],
                metadata: ArchitectureMetadata::new(),
            },
            capabilities: CapabilityManifest::default(),
        }
    }

    // ==========================================================================
    // HelpLevel Tests
    // ==========================================================================

    #[test]
    fn test_help_level_default() {
        let level = HelpLevel::default();
        assert_eq!(level, HelpLevel::Brief);
    }

    #[test]
    fn test_help_level_from_arg_help() {
        assert_eq!(HelpLevel::from_arg("--help"), Some(HelpLevel::Brief));
        assert_eq!(HelpLevel::from_arg("-h"), Some(HelpLevel::Brief));
    }

    #[test]
    fn test_help_level_from_arg_more() {
        assert_eq!(HelpLevel::from_arg("--help-more"), Some(HelpLevel::More));
    }

    #[test]
    fn test_help_level_from_arg_implementation() {
        assert_eq!(
            HelpLevel::from_arg("--help-implementation"),
            Some(HelpLevel::Implementation)
        );
        assert_eq!(
            HelpLevel::from_arg("--help-impl"),
            Some(HelpLevel::Implementation)
        );
    }

    #[test]
    fn test_help_level_from_arg_unknown() {
        assert_eq!(HelpLevel::from_arg("--version"), None);
        assert_eq!(HelpLevel::from_arg("-v"), None);
        assert_eq!(HelpLevel::from_arg("help"), None);
        assert_eq!(HelpLevel::from_arg(""), None);
        assert_eq!(HelpLevel::from_arg("--Help"), None); // case-sensitive
    }

    #[test]
    fn test_help_level_from_args_finds_first() {
        let args = vec!["myapp", "--help"];
        assert_eq!(HelpLevel::from_args(&args), Some(HelpLevel::Brief));
    }

    #[test]
    fn test_help_level_from_args_finds_help_more() {
        let args = vec!["myapp", "--debug", "--help-more", "--verbose"];
        assert_eq!(HelpLevel::from_args(&args), Some(HelpLevel::More));
    }

    #[test]
    fn test_help_level_from_args_finds_implementation() {
        let args = vec!["myapp", "--help-implementation"];
        assert_eq!(HelpLevel::from_args(&args), Some(HelpLevel::Implementation));
    }

    #[test]
    fn test_help_level_from_args_no_help() {
        let args = vec!["myapp", "--config", "file.toml"];
        assert_eq!(HelpLevel::from_args(&args), None);
    }

    #[test]
    fn test_help_level_from_args_empty() {
        let args: Vec<&str> = vec![];
        assert_eq!(HelpLevel::from_args(&args), None);
    }

    #[test]
    fn test_help_level_from_args_with_strings() {
        let args: Vec<String> = vec!["myapp".into(), "-h".into()];
        assert_eq!(HelpLevel::from_args(&args), Some(HelpLevel::Brief));
    }

    #[test]
    fn test_help_level_is_help_requested_true() {
        let args = vec!["myapp", "--help"];
        assert!(HelpLevel::is_help_requested(&args));
    }

    #[test]
    fn test_help_level_is_help_requested_false() {
        let args = vec!["myapp", "--run"];
        assert!(!HelpLevel::is_help_requested(&args));
    }

    #[test]
    fn test_help_level_is_help_requested_empty() {
        let args: Vec<&str> = vec![];
        assert!(!HelpLevel::is_help_requested(&args));
    }

    #[test]
    #[allow(clippy::clone_on_copy)]
    fn test_help_level_debug_clone_copy() {
        let level = HelpLevel::More;
        let cloned = level.clone();
        let copied = level;
        assert_eq!(level, cloned);
        assert_eq!(level, copied);
        // Test Debug
        let debug_str = format!("{:?}", level);
        assert!(debug_str.contains("More"));
    }

    // ==========================================================================
    // HelpGenerator Tests
    // ==========================================================================

    #[test]
    fn test_help_generator_new() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        assert!(generator.app_name.is_none());
        assert!(generator.app_version.is_none());
        assert!(generator.app_description.is_none());
    }

    #[test]
    fn test_help_generator_with_app_name() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection).with_app_name("MyApp");
        assert_eq!(generator.app_name, Some("MyApp".to_string()));
    }

    #[test]
    fn test_help_generator_with_app_version() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection).with_app_version("2.0.0");
        assert_eq!(generator.app_version, Some("2.0.0".to_string()));
    }

    #[test]
    fn test_help_generator_with_app_description() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection).with_app_description("A great app");
        assert_eq!(generator.app_description, Some("A great app".to_string()));
    }

    #[test]
    fn test_help_generator_builder_chain() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection)
            .with_app_name("ChainedApp")
            .with_app_version("3.0.0")
            .with_app_description("Built with builder pattern");

        assert_eq!(generator.app_name, Some("ChainedApp".to_string()));
        assert_eq!(generator.app_version, Some("3.0.0".to_string()));
        assert_eq!(
            generator.app_description,
            Some("Built with builder pattern".to_string())
        );
    }

    #[test]
    fn test_help_generator_app_name_fallback_to_graph_name() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        assert_eq!(generator.app_name(), "TestGraph");
    }

    #[test]
    fn test_help_generator_app_name_fallback_to_default() {
        let introspection = create_minimal_introspection();
        let generator = HelpGenerator::new(introspection);
        assert_eq!(generator.app_name(), "DashFlow Application");
    }

    #[test]
    fn test_help_generator_app_name_override() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection).with_app_name("OverriddenName");
        assert_eq!(generator.app_name(), "OverriddenName");
    }

    #[test]
    fn test_help_generator_app_version_default() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        assert_eq!(generator.app_version(), "1.0.0");
    }

    #[test]
    fn test_help_generator_app_version_override() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection).with_app_version("5.0.0");
        assert_eq!(generator.app_version(), "5.0.0");
    }

    #[test]
    fn test_help_generator_app_description_from_metadata() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        assert_eq!(generator.app_description(), "A test AI agent");
    }

    #[test]
    fn test_help_generator_app_description_default() {
        let introspection = create_minimal_introspection();
        let generator = HelpGenerator::new(introspection);
        assert_eq!(
            generator.app_description(),
            "A DashFlow-powered AI agent"
        );
    }

    #[test]
    fn test_help_generator_app_description_override() {
        let introspection = create_test_introspection();
        let generator =
            HelpGenerator::new(introspection).with_app_description("Custom description");
        assert_eq!(generator.app_description(), "Custom description");
    }

    #[test]
    fn test_help_generator_generate_brief() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let help = generator.generate(HelpLevel::Brief);

        assert!(help.contains("TestGraph"));
        assert!(help.contains("CAPABILITIES:"));
        assert!(help.contains("USAGE:"));
        assert!(help.contains("OPTIONS:"));
        assert!(help.contains("--help"));
        assert!(help.contains("--help-more"));
        assert!(help.contains("--help-implementation"));
    }

    #[test]
    fn test_help_generator_generate_more() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let help = generator.generate(HelpLevel::More);

        assert!(help.contains("ARCHITECTURE:"));
        assert!(help.contains("NODES:"));
        assert!(help.contains("DASHFLOW FEATURES USED:"));
        assert!(help.contains("EXECUTION FLOW:"));
        // Should include brief content too
        assert!(help.contains("CAPABILITIES:"));
    }

    #[test]
    fn test_help_generator_generate_implementation() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let help = generator.generate(HelpLevel::Implementation);

        assert!(help.contains("IMPLEMENTATION DETAILS:"));
        assert!(help.contains("NODE VERSIONS:"));
        assert!(help.contains("DASHFLOW VERSION:"));
        assert!(help.contains("INTERNAL APIS:"));
        assert!(help.contains("DEPENDENCIES"));
        // Should include more and brief content too
        assert!(help.contains("ARCHITECTURE:"));
        assert!(help.contains("CAPABILITIES:"));
    }

    #[test]
    fn test_help_generator_list_capabilities_brief_with_tools() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let capabilities = generator.list_capabilities_brief();

        assert!(capabilities.contains("search"));
        assert!(capabilities.contains("Search the web"));
        assert!(capabilities.contains("write_file"));
    }

    #[test]
    fn test_help_generator_list_capabilities_brief_no_tools() {
        let introspection = create_minimal_introspection();
        let generator = HelpGenerator::new(introspection);
        let capabilities = generator.list_capabilities_brief();

        assert!(capabilities.contains("Graph-based AI orchestration"));
    }

    #[test]
    fn test_help_generator_list_nodes_with_descriptions() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let nodes = generator.list_nodes_with_descriptions();

        assert!(nodes.contains("start"));
        assert!(nodes.contains("Entry point node"));
        assert!(nodes.contains("process"));
        assert!(nodes.contains("decision"));
        assert!(nodes.contains("function")); // node type
    }

    #[test]
    fn test_help_generator_list_dashflow_features_used() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let features = generator.list_dashflow_features_used();

        assert!(features.contains("StateGraph"));
        assert!(features.contains("Core graph orchestration"));
        assert!(features.contains("Checkpoint"));
    }

    #[test]
    fn test_help_generator_list_dashflow_features_empty() {
        let introspection = create_minimal_introspection();
        let generator = HelpGenerator::new(introspection);
        let features = generator.list_dashflow_features_used();

        assert!(features.contains("StateGraph (core orchestration)"));
    }

    #[test]
    fn test_help_generator_explain_execution_flow() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let flow = generator.explain_execution_flow();

        assert!(flow.contains("Execution starts at node 'start'"));
        assert!(flow.contains("Decision points:"));
        assert!(flow.contains("decision"));
        assert!(flow.contains("Execution ends at:"));
    }

    #[test]
    fn test_help_generator_list_node_versions() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let versions = generator.list_node_versions();

        // Default version should be 1.0.0
        assert!(versions.contains("v1.0.0"));
    }

    #[test]
    fn test_help_generator_list_dependencies() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let deps = generator.list_dependencies();

        // Only DashFlow crates should be listed
        assert!(deps.contains("dashflow"));
        assert!(deps.contains("Core framework"));
        // Non-DashFlow crates should not be listed
        assert!(!deps.contains("tokio"));
    }

    #[test]
    fn test_help_generator_list_internal_apis() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let apis = generator.list_internal_apis();

        assert!(apis.contains("StateGraph"));
        assert!(apis.contains("CompiledGraph"));
        assert!(apis.contains("Introspection"));
        assert!(apis.contains("Platform"));
    }

    #[test]
    fn test_help_generator_debug_clone() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let cloned = generator.clone();

        // Both should produce the same output
        assert_eq!(
            generator.generate(HelpLevel::Brief),
            cloned.generate(HelpLevel::Brief)
        );

        // Test Debug
        let debug_str = format!("{:?}", generator);
        assert!(debug_str.contains("HelpGenerator"));
    }

    // ==========================================================================
    // CliHelpConfig Tests
    // ==========================================================================

    #[test]
    fn test_cli_help_config_default() {
        let config = CliHelpConfig::default();
        assert!(config.app_name.is_none());
        assert!(config.app_version.is_none());
        assert!(config.app_description.is_none());
        assert!(!config.output_to_stderr);
    }

    #[test]
    fn test_cli_help_config_new() {
        let config = CliHelpConfig::new();
        assert!(config.app_name.is_none());
        assert!(!config.output_to_stderr);
    }

    #[test]
    fn test_cli_help_config_with_app_name() {
        let config = CliHelpConfig::new().with_app_name("TestApp");
        assert_eq!(config.app_name, Some("TestApp".to_string()));
    }

    #[test]
    fn test_cli_help_config_with_app_version() {
        let config = CliHelpConfig::new().with_app_version("1.2.3");
        assert_eq!(config.app_version, Some("1.2.3".to_string()));
    }

    #[test]
    fn test_cli_help_config_with_app_description() {
        let config = CliHelpConfig::new().with_app_description("A test application");
        assert_eq!(
            config.app_description,
            Some("A test application".to_string())
        );
    }

    #[test]
    fn test_cli_help_config_output_to_stderr() {
        let config = CliHelpConfig::new().output_to_stderr();
        assert!(config.output_to_stderr);
    }

    #[test]
    fn test_cli_help_config_builder_chain() {
        let config = CliHelpConfig::new()
            .with_app_name("ChainApp")
            .with_app_version("4.0.0")
            .with_app_description("Chained config")
            .output_to_stderr();

        assert_eq!(config.app_name, Some("ChainApp".to_string()));
        assert_eq!(config.app_version, Some("4.0.0".to_string()));
        assert_eq!(config.app_description, Some("Chained config".to_string()));
        assert!(config.output_to_stderr);
    }

    #[test]
    fn test_cli_help_config_debug_clone() {
        let config = CliHelpConfig::new().with_app_name("DebugTest");
        let cloned = config.clone();
        assert_eq!(config.app_name, cloned.app_name);

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("CliHelpConfig"));
        assert!(debug_str.contains("DebugTest"));
    }

    // ==========================================================================
    // CliHelpResult Tests
    // ==========================================================================

    #[test]
    fn test_cli_help_result_continue_should_exit() {
        let result = CliHelpResult::Continue;
        assert!(!result.should_exit());
    }

    #[test]
    fn test_cli_help_result_continue_should_continue() {
        let result = CliHelpResult::Continue;
        assert!(result.should_continue());
    }

    #[test]
    fn test_cli_help_result_displayed_should_exit() {
        let result = CliHelpResult::Displayed(HelpLevel::Brief);
        assert!(result.should_exit());
    }

    #[test]
    fn test_cli_help_result_displayed_should_continue() {
        let result = CliHelpResult::Displayed(HelpLevel::More);
        assert!(!result.should_continue());
    }

    #[test]
    fn test_cli_help_result_displayed_variants() {
        let brief = CliHelpResult::Displayed(HelpLevel::Brief);
        let more = CliHelpResult::Displayed(HelpLevel::More);
        let impl_level = CliHelpResult::Displayed(HelpLevel::Implementation);

        assert!(brief.should_exit());
        assert!(more.should_exit());
        assert!(impl_level.should_exit());
    }

    #[test]
    #[allow(clippy::clone_on_copy)]
    fn test_cli_help_result_debug_clone_copy() {
        let result = CliHelpResult::Displayed(HelpLevel::Brief);
        let cloned = result.clone();
        let copied = result;

        assert_eq!(result, cloned);
        assert_eq!(result, copied);

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Displayed"));
    }

    #[test]
    fn test_cli_help_result_eq() {
        assert_eq!(CliHelpResult::Continue, CliHelpResult::Continue);
        assert_eq!(
            CliHelpResult::Displayed(HelpLevel::Brief),
            CliHelpResult::Displayed(HelpLevel::Brief)
        );
        assert_ne!(CliHelpResult::Continue, CliHelpResult::Displayed(HelpLevel::Brief));
        assert_ne!(
            CliHelpResult::Displayed(HelpLevel::Brief),
            CliHelpResult::Displayed(HelpLevel::More)
        );
    }

    // ==========================================================================
    // process_cli_help Tests
    // ==========================================================================

    #[test]
    fn test_process_cli_help_no_help_flag() {
        let args = vec!["myapp", "--config", "file.toml"];
        let introspection = create_test_introspection();

        let result = process_cli_help(args, introspection, None);
        assert_eq!(result, CliHelpResult::Continue);
    }

    #[test]
    fn test_process_cli_help_empty_args() {
        let args: Vec<&str> = vec![];
        let introspection = create_test_introspection();

        let result = process_cli_help(args, introspection, None);
        assert_eq!(result, CliHelpResult::Continue);
    }

    #[test]
    fn test_process_cli_help_brief() {
        let args = vec!["myapp", "--help"];
        let introspection = create_test_introspection();

        let result = process_cli_help(args, introspection, None);
        assert_eq!(result, CliHelpResult::Displayed(HelpLevel::Brief));
    }

    #[test]
    fn test_process_cli_help_more() {
        let args = vec!["myapp", "--help-more"];
        let introspection = create_test_introspection();

        let result = process_cli_help(args, introspection, None);
        assert_eq!(result, CliHelpResult::Displayed(HelpLevel::More));
    }

    #[test]
    fn test_process_cli_help_implementation() {
        let args = vec!["myapp", "--help-impl"];
        let introspection = create_test_introspection();

        let result = process_cli_help(args, introspection, None);
        assert_eq!(result, CliHelpResult::Displayed(HelpLevel::Implementation));
    }

    #[test]
    fn test_process_cli_help_with_config() {
        let args = vec!["myapp", "-h"];
        let introspection = create_test_introspection();
        let config = CliHelpConfig::new()
            .with_app_name("ConfiguredApp")
            .with_app_version("9.9.9");

        let result = process_cli_help(args, introspection, Some(config));
        assert_eq!(result, CliHelpResult::Displayed(HelpLevel::Brief));
    }

    #[test]
    fn test_process_cli_help_with_string_args() {
        let args: Vec<String> = vec!["myapp".into(), "--help-more".into()];
        let introspection = create_test_introspection();

        let result = process_cli_help(args, introspection, None);
        assert_eq!(result, CliHelpResult::Displayed(HelpLevel::More));
    }

    #[test]
    fn test_process_cli_help_help_flag_in_middle() {
        let args = vec!["myapp", "--verbose", "-h", "--config", "file.toml"];
        let introspection = create_test_introspection();

        let result = process_cli_help(args, introspection, None);
        assert_eq!(result, CliHelpResult::Displayed(HelpLevel::Brief));
    }

    #[test]
    fn test_process_cli_help_first_help_wins() {
        // If multiple help flags present, first one wins
        let args = vec!["myapp", "--help", "--help-more", "--help-implementation"];
        let introspection = create_test_introspection();

        let result = process_cli_help(args, introspection, None);
        assert_eq!(result, CliHelpResult::Displayed(HelpLevel::Brief));
    }

    // ==========================================================================
    // Integration Tests
    // ==========================================================================

    #[test]
    fn test_full_workflow_no_help() {
        let args = vec!["myapp", "run", "--input", "data.txt"];
        let introspection = create_test_introspection();

        let result = process_cli_help(args, introspection, None);

        assert!(result.should_continue());
        assert!(!result.should_exit());
    }

    #[test]
    fn test_full_workflow_with_help() {
        let args = vec!["myapp", "--help"];
        let introspection = create_test_introspection();
        let config = CliHelpConfig::new()
            .with_app_name("MyAIAgent")
            .with_app_version("1.0.0")
            .with_app_description("An intelligent assistant");

        let result = process_cli_help(args, introspection, Some(config));

        assert!(result.should_exit());
        assert!(!result.should_continue());
    }

    #[test]
    fn test_minimal_introspection_generates_valid_help() {
        let introspection = create_minimal_introspection();
        let generator = HelpGenerator::new(introspection);

        // All three levels should produce non-empty output
        let brief = generator.generate(HelpLevel::Brief);
        let more = generator.generate(HelpLevel::More);
        let implementation = generator.generate(HelpLevel::Implementation);

        assert!(!brief.is_empty());
        assert!(!more.is_empty());
        assert!(!implementation.is_empty());

        // More should contain Brief content
        assert!(more.len() > brief.len());
        // Implementation should contain More content
        assert!(implementation.len() > more.len());
    }

    #[test]
    fn test_help_contains_all_sections() {
        let introspection = create_test_introspection();
        let generator = HelpGenerator::new(introspection);
        let help = generator.generate(HelpLevel::Implementation);

        // Check all major sections are present
        let sections = [
            "CAPABILITIES:",
            "USAGE:",
            "OPTIONS:",
            "ARCHITECTURE:",
            "NODES:",
            "DASHFLOW FEATURES USED:",
            "EXECUTION FLOW:",
            "IMPLEMENTATION DETAILS:",
            "NODE VERSIONS:",
            "DASHFLOW VERSION:",
            "INTERNAL APIS:",
            "DEPENDENCIES",
        ];

        for section in &sections {
            assert!(
                help.contains(section),
                "Missing section: {}",
                section
            );
        }
    }

    #[test]
    fn test_capabilities_limit_to_10() {
        // Create introspection with more than 10 tools
        let mut introspection = create_test_introspection();
        introspection.capabilities.tools = (0..15)
            .map(|i| ToolManifest {
                name: format!("tool_{}", i),
                description: format!("Tool number {}", i),
                category: None,
                parameters: vec![],
                returns: None,
                has_side_effects: false,
                requires_confirmation: false,
                metadata: HashMap::new(),
            })
            .collect();

        let generator = HelpGenerator::new(introspection);
        let capabilities = generator.list_capabilities_brief();

        // Should contain first 10 tools
        assert!(capabilities.contains("tool_0"));
        assert!(capabilities.contains("tool_9"));
        // Should not contain tool_10 and beyond (limited to 10)
        assert!(!capabilities.contains("tool_10"));
    }
}
