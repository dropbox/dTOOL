// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Platform Registry - DashFlow Self-Knowledge
//!
//! This module provides AIs with knowledge about the DashFlow platform itself:
//! - What APIs does DashFlow provide?
//! - What features are available?
//! - What crates exist in the ecosystem?
//!
//! ## Overview
//!
//! AI agents need to understand the platform they're built on:
//! - "What is DashFlow?"
//! - "What can I build with it?"
//! - "What features are available to me?"
//!
//! This is distinct from introspection (understanding runtime behavior) - this is about
//! understanding the platform's capabilities at a structural level.
//!
//! ## Platform API Registry
//!
//! ```rust,ignore
//! use dashflow::platform_registry::PlatformRegistry;
//!
//! // Get platform capabilities
//! let platform = PlatformRegistry::discover();
//!
//! // AI can ask: "What can DashFlow do?"
//! for module in &platform.modules {
//!     println!("Module: {} - {}", module.name, module.description);
//! }
//!
//! // AI can ask: "How do I create a graph?"
//! if let Some(api) = platform.find_api("StateGraph::new") {
//!     println!("Usage: {}", api.example.as_deref().unwrap_or("No example"));
//! }
//!
//! // Export as JSON for AI consumption
//! let json = platform.to_json().unwrap();
//! ```

use serde::{Deserialize, Serialize};

// Submodules
pub mod dependency_analysis;
pub mod execution_flow;
pub mod node_purpose;

// Re-exports for backwards compatibility
pub use dependency_analysis::{
    parse_cargo_toml, CrateDependency, DependencyAnalysis, DependencyAnalysisBuilder,
    DependencyCategory, DependencyMetadata,
};
pub use execution_flow::{
    generate_flow_description, DecisionPath, DecisionPoint, DecisionType, ExecutionFlow,
    ExecutionFlowBuilder, ExecutionFlowMetadata, ExecutionPath, LoopStructure, LoopType,
};
pub use node_purpose::{
    infer_node_type, ApiUsage, ExternalCall, ExternalCallType, NodePurpose, NodePurposeBuilder,
    NodePurposeCollection, NodePurposeCollectionMetadata, NodePurposeMetadata, NodeType,
    StateFieldUsage,
};

// Include auto-generated module discovery from build.rs
include!(concat!(env!("OUT_DIR"), "/discovered_modules.rs"));

/// Platform registry - complete DashFlow platform knowledge
///
/// This is the primary data structure for AI platform awareness. It contains
/// everything an AI needs to understand what DashFlow can do.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::platform_registry::PlatformRegistry;
///
/// let platform = PlatformRegistry::discover();
///
/// // Check what features are available
/// for feature in &platform.features {
///     println!("Feature: {} - {}", feature.name, feature.description);
/// }
///
/// // Find APIs by name
/// if let Some(api) = platform.find_api("compile") {
///     println!("Found: {}", api.description);
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformRegistry {
    /// DashFlow version
    pub version: String,
    /// All modules in the platform
    pub modules: Vec<ModuleInfo>,
    /// All features available
    pub features: Vec<FeatureInfo>,
    /// All crates in the ecosystem
    pub crates: Vec<CrateInfo>,
    /// Platform metadata
    pub metadata: PlatformMetadata,
}

impl PlatformRegistry {
    /// Create a new platform registry builder
    #[must_use]
    pub fn builder() -> PlatformRegistryBuilder {
        PlatformRegistryBuilder::new()
    }

    /// Discover platform capabilities
    ///
    /// This builds a complete registry of all DashFlow platform capabilities
    /// by analyzing the crate structure and public APIs.
    #[must_use]
    pub fn discover() -> Self {
        let mut builder = PlatformRegistryBuilder::new();

        // Core module
        builder.add_module(
            ModuleInfo::builder()
                .name("core")
                .description("Core framework types and traits")
                .add_api(ApiInfo::new(
                    "StateGraph::new",
                    "Create a new state graph for building workflows",
                    "fn new() -> StateGraph<S>",
                    Some("let graph = StateGraph::new();"),
                ))
                .add_api(ApiInfo::new(
                    "StateGraph::add_node",
                    "Add a node to the graph",
                    "fn add_node(&mut self, name: &str, node: impl Node<S>)",
                    Some("graph.add_node(\"processor\", process_fn);"),
                ))
                .add_api(ApiInfo::new(
                    "StateGraph::add_edge",
                    "Add an edge between nodes",
                    "fn add_edge(&mut self, from: &str, to: &str)",
                    Some("graph.add_edge(\"input\", \"processor\");"),
                ))
                .add_api(ApiInfo::new(
                    "StateGraph::add_conditional_edges",
                    "Add conditional routing between nodes",
                    "fn add_conditional_edges(&mut self, source: &str, condition: impl Fn(&S) -> String)",
                    Some("graph.add_conditional_edges(\"router\", |s| s.next.clone());"),
                ))
                .add_api(ApiInfo::new(
                    "StateGraph::set_entry_point",
                    "Set the entry point for graph execution",
                    "fn set_entry_point(&mut self, node: &str)",
                    Some("graph.set_entry_point(\"input\");"),
                ))
                .add_api(ApiInfo::new(
                    "StateGraph::compile",
                    "Compile the graph for execution",
                    "fn compile(self) -> Result<CompiledGraph<S>>",
                    Some("let app = graph.compile()?;"),
                ))
                .add_api(ApiInfo::new(
                    "CompiledGraph::invoke",
                    "Execute the graph with initial state",
                    "async fn invoke(&self, state: S) -> Result<S>",
                    Some("let result = app.invoke(initial_state).await?;"),
                ))
                .add_api(ApiInfo::new(
                    "CompiledGraph::stream",
                    "Stream execution events",
                    "fn stream(&self, state: S) -> impl Stream<Item = StreamEvent<S>>",
                    Some("let mut stream = app.stream(state);"),
                ))
                .build(),
        );

        // GraphBuilder module (fluent API)
        builder.add_module(
            ModuleInfo::builder()
                .name("builder")
                .description("Fluent graph builder API")
                .add_api(ApiInfo::new(
                    "GraphBuilder::new",
                    "Create a new fluent graph builder",
                    "fn new() -> GraphBuilder<S>",
                    Some("let mut graph = GraphBuilder::new();"),
                ))
                .add_api(ApiInfo::new(
                    "GraphBuilder::add_node",
                    "Add a node with fluent chaining",
                    "fn add_node(&mut self, name: &str, node: impl Node<S>) -> &mut Self",
                    Some("graph.add_node(\"a\", fn_a).add_node(\"b\", fn_b);"),
                ))
                .build(),
        );

        // Checkpoint module
        builder.add_module(
            ModuleInfo::builder()
                .name("checkpoint")
                .description("State persistence and checkpointing")
                .add_api(ApiInfo::new(
                    "MemoryCheckpointer::new",
                    "Create an in-memory checkpointer",
                    "fn new() -> MemoryCheckpointer",
                    Some("let cp = MemoryCheckpointer::new();"),
                ))
                .add_api(ApiInfo::new(
                    "SqliteCheckpointer::new",
                    "Create a SQLite-backed checkpointer",
                    "async fn new(path: &str) -> Result<SqliteCheckpointer>",
                    Some("let cp = SqliteCheckpointer::new(\"state.db\").await?;"),
                ))
                .add_api(ApiInfo::new(
                    "FileCheckpointer::new",
                    "Create a file-based checkpointer",
                    "fn new(path: impl AsRef<Path>) -> FileCheckpointer",
                    Some("let cp = FileCheckpointer::new(\"./checkpoints\");"),
                ))
                .build(),
        );

        // Introspection module
        builder.add_module(
            ModuleInfo::builder()
                .name("introspection")
                .description("AI self-awareness and execution monitoring")
                .add_api(ApiInfo::new(
                    "GraphManifest",
                    "Complete graph structure for AI consumption",
                    "struct GraphManifest { nodes, edges, entry_point, ... }",
                    Some("let manifest = graph.manifest();"),
                ))
                .add_api(ApiInfo::new(
                    "ExecutionContext",
                    "Runtime context available during execution",
                    "struct ExecutionContext { current_node, iteration, ... }",
                    Some("if context.iteration > 10 { break; }"),
                ))
                .add_api(ApiInfo::new(
                    "ExecutionTrace",
                    "Record of execution history",
                    "struct ExecutionTrace { nodes_executed, total_duration, ... }",
                    Some("let trace = graph.get_execution_trace(thread_id).await?;"),
                ))
                .add_api(ApiInfo::new(
                    "PerformanceMetrics",
                    "Real-time performance monitoring",
                    "struct PerformanceMetrics { latency_ms, tokens_per_second, ... }",
                    Some("let metrics = graph.performance_monitor();"),
                ))
                .add_api(ApiInfo::new(
                    "ResourceUsage",
                    "Resource consumption tracking",
                    "struct ResourceUsage { tokens_used, cost_usd, ... }",
                    Some("let usage = graph.resource_usage(thread_id).await?;"),
                ))
                .add_api(ApiInfo::new(
                    "PatternAnalysis",
                    "Learn patterns from execution history",
                    "struct PatternAnalysis { patterns, ... }",
                    Some("let patterns = trace.learn_patterns();"),
                ))
                .build(),
        );

        // Streaming module
        builder.add_module(
            ModuleInfo::builder()
                .name("streaming")
                .description("Real-time execution streaming")
                .add_api(ApiInfo::new(
                    "DashStreamCallback",
                    "Callback for streaming execution events",
                    "struct DashStreamCallback { ... }",
                    Some("let callback = DashStreamCallback::new(config);"),
                ))
                .add_api(ApiInfo::new(
                    "StreamEvent",
                    "Events emitted during streaming",
                    "enum StreamEvent { NodeStart, NodeEnd, StateUpdate, ... }",
                    Some("while let Some(event) = stream.next().await { ... }"),
                ))
                .build(),
        );

        // Optimization module
        builder.add_module(
            ModuleInfo::builder()
                .name("optimize")
                .description("Prompt optimization and A/B testing")
                .add_api(ApiInfo::new(
                    "DashOptimize",
                    "Native prompt optimization framework",
                    "struct DashOptimize { ... }",
                    Some("let optimizer = DashOptimize::new(config);"),
                ))
                .add_api(ApiInfo::new(
                    "ABTestRunner",
                    "A/B testing for prompts and models",
                    "struct ABTestRunner { ... }",
                    Some("let test = ABTestRunner::new(variants);"),
                ))
                .build(),
        );

        // Quality module
        builder.add_module(
            ModuleInfo::builder()
                .name("quality")
                .description("Quality gates and validation")
                .add_api(ApiInfo::new(
                    "QualityGate",
                    "Quality gate for response validation",
                    "struct QualityGate { ... }",
                    Some("let gate = QualityGate::new(config);"),
                ))
                .add_api(ApiInfo::new(
                    "ResponseValidator",
                    "Validate LLM responses",
                    "trait ResponseValidator { fn validate(&self, response: &str) -> ValidationResult }",
                    None,
                ))
                .build(),
        );

        // Add features with detailed information
        builder
            .add_feature(FeatureInfo::with_details(
                "graph_orchestration",
                "Graph-based workflow orchestration",
                "Build complex workflows with directed graphs, cycles, and conditional routing",
                FeatureDetails::builder().enabled_by_default(true).build(),
            ))
            .add_feature(FeatureInfo::with_details(
                "checkpointing",
                "State persistence and recovery",
                "Save and restore execution state with multiple backends",
                FeatureDetails::builder()
                    .backends(vec![
                        "Memory",
                        "SQLite",
                        "File",
                        "Redis",
                        "PostgreSQL",
                        "S3",
                    ])
                    .enabled_by_default(true)
                    .build(),
            ))
            .add_feature(FeatureInfo::with_details(
                "streaming",
                "Real-time execution streaming",
                "Stream execution events for real-time UI updates and monitoring",
                FeatureDetails::builder()
                    .backends(vec!["WebSocket", "SSE", "Callback"])
                    .enabled_by_default(true)
                    .build(),
            ))
            .add_feature(FeatureInfo::with_details(
                "introspection",
                "AI self-awareness",
                "Enable AIs to understand their own structure, state, and performance",
                FeatureDetails::builder().enabled_by_default(true).build(),
            ))
            .add_feature(FeatureInfo::with_details(
                "optimization",
                "Prompt optimization",
                "Optimize prompts with A/B testing, quality gates, and automatic tuning",
                FeatureDetails::builder()
                    .algorithms(vec![
                        "MIPRO",
                        "DashOptimize",
                        "BootstrapFewShot",
                        "GeneticOptimizer",
                    ])
                    .build(),
            ))
            .add_feature(FeatureInfo::with_details(
                "human_in_the_loop",
                "Human approval workflows",
                "Add human approval steps to agent execution",
                FeatureDetails::builder().build(),
            ))
            .add_feature(FeatureInfo::with_details(
                "parallel_execution",
                "Parallel node execution",
                "Execute multiple nodes in parallel with automatic fan-out/fan-in",
                FeatureDetails::builder().enabled_by_default(true).build(),
            ))
            .add_feature(FeatureInfo::with_details(
                "subgraphs",
                "Nested graph composition",
                "Compose graphs from smaller subgraphs for modularity",
                FeatureDetails::builder().enabled_by_default(true).build(),
            ))
            .add_feature(FeatureInfo::with_details(
                "llm_providers",
                "LLM Provider Integrations",
                "Connect to various LLM providers for inference",
                FeatureDetails::builder()
                    .supported(vec![
                        "OpenAI",
                        "Anthropic",
                        "AWS Bedrock",
                        "Google Gemini",
                        "Ollama",
                        "Azure OpenAI",
                        "Cohere",
                        "Mistral",
                    ])
                    .build(),
            ))
            .add_feature(FeatureInfo::with_details(
                "vector_stores",
                "Vector Store Integrations",
                "Connect to vector databases for semantic search and RAG",
                FeatureDetails::builder()
                    .supported(vec![
                        "Chroma",
                        "Pinecone",
                        "Qdrant",
                        "PostgreSQL pgvector",
                        "Redis",
                        "Milvus",
                        "Weaviate",
                    ])
                    .build(),
            ))
            .add_feature(FeatureInfo::with_details(
                "tools",
                "Agent Tools",
                "Built-in tools for agent capabilities",
                FeatureDetails::builder()
                    .supported(vec!["Shell", "File", "Git", "HTTP", "Search", "Calculator"])
                    .build(),
            ))
            .add_feature(FeatureInfo::with_details(
                "embeddings",
                "Embedding Providers",
                "Generate embeddings for semantic search",
                FeatureDetails::builder()
                    .supported(vec![
                        "OpenAI",
                        "Cohere",
                        "HuggingFace",
                        "Sentence Transformers",
                    ])
                    .build(),
            ));

        // Add crate information
        builder
            .add_crate(CrateInfo::new(
                "dashflow",
                "Core orchestration framework",
                CrateCategory::Core,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-openai",
                "OpenAI integration (GPT-4, GPT-3.5)",
                CrateCategory::LlmProvider,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-anthropic",
                "Anthropic integration (Claude)",
                CrateCategory::LlmProvider,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-bedrock",
                "AWS Bedrock integration",
                CrateCategory::LlmProvider,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-gemini",
                "Google Gemini integration",
                CrateCategory::LlmProvider,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-ollama",
                "Ollama local model integration",
                CrateCategory::LlmProvider,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-chroma",
                "Chroma vector store",
                CrateCategory::VectorStore,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-pinecone",
                "Pinecone vector store",
                CrateCategory::VectorStore,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-qdrant",
                "Qdrant vector store",
                CrateCategory::VectorStore,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-pgvector",
                "PostgreSQL pgvector integration",
                CrateCategory::VectorStore,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-shell-tool",
                "Safe shell command execution",
                CrateCategory::Tool,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-file-tool",
                "File system operations",
                CrateCategory::Tool,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-git-tool",
                "Git repository operations",
                CrateCategory::Tool,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-http-requests",
                "HTTP request tool",
                CrateCategory::Tool,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-redis-checkpointer",
                "Redis checkpoint backend",
                CrateCategory::Checkpointer,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-postgres-checkpointer",
                "PostgreSQL checkpoint backend",
                CrateCategory::Checkpointer,
            ))
            .add_crate(CrateInfo::new(
                "dashflow-s3-checkpointer",
                "S3 checkpoint backend",
                CrateCategory::Checkpointer,
            ));

        builder.build()
    }

    /// Convert registry to JSON string for AI consumption
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convert registry to compact JSON (smaller size)
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse registry from JSON string
    ///
    /// # Errors
    ///
    /// Returns error if deserialization fails
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Find an API by name (case-insensitive search)
    #[must_use]
    pub fn find_api(&self, name: &str) -> Option<&ApiInfo> {
        let name_lower = name.to_lowercase();
        for module in &self.modules {
            for api in &module.apis {
                if api.function.to_lowercase().contains(&name_lower) {
                    return Some(api);
                }
            }
        }
        None
    }

    /// Find all APIs matching a pattern
    #[must_use]
    pub fn search_apis(&self, pattern: &str) -> Vec<&ApiInfo> {
        let pattern_lower = pattern.to_lowercase();
        let mut results = Vec::new();
        for module in &self.modules {
            for api in &module.apis {
                if api.function.to_lowercase().contains(&pattern_lower)
                    || api.description.to_lowercase().contains(&pattern_lower)
                {
                    results.push(api);
                }
            }
        }
        results
    }

    /// Get all APIs in a module
    #[must_use]
    pub fn apis_in_module(&self, module_name: &str) -> Vec<&ApiInfo> {
        self.modules
            .iter()
            .find(|m| m.name == module_name)
            .map(|m| m.apis.iter().collect())
            .unwrap_or_default()
    }

    /// Get all modules
    #[must_use]
    pub fn module_names(&self) -> Vec<&str> {
        self.modules.iter().map(|m| m.name.as_str()).collect()
    }

    /// Check if a feature is available
    #[must_use]
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f.name == feature)
    }

    /// Get all features
    #[must_use]
    pub fn feature_names(&self) -> Vec<&str> {
        self.features.iter().map(|f| f.name.as_str()).collect()
    }

    /// Get a feature by name
    #[must_use]
    pub fn get_feature(&self, name: &str) -> Option<&FeatureInfo> {
        self.features.iter().find(|f| f.name == name)
    }

    /// Get features enabled by default
    #[must_use]
    pub fn default_features(&self) -> Vec<&FeatureInfo> {
        self.features
            .iter()
            .filter(|f| {
                f.details
                    .as_ref()
                    .map(|d| d.enabled_by_default)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Get features with available backends
    #[must_use]
    pub fn features_with_backends(&self) -> Vec<&FeatureInfo> {
        self.features
            .iter()
            .filter(|f| f.backends().is_some())
            .collect()
    }

    /// Get all supported LLM providers
    #[must_use]
    pub fn supported_llm_providers(&self) -> Vec<&str> {
        self.get_feature("llm_providers")
            .and_then(|f| f.supported())
            .map(|s| s.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Get all supported vector stores
    #[must_use]
    pub fn supported_vector_stores(&self) -> Vec<&str> {
        self.get_feature("vector_stores")
            .and_then(|f| f.supported())
            .map(|s| s.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Get all supported tools
    #[must_use]
    pub fn supported_tools(&self) -> Vec<&str> {
        self.get_feature("tools")
            .and_then(|f| f.supported())
            .map(|s| s.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Get all checkpoint backends
    #[must_use]
    pub fn checkpoint_backends(&self) -> Vec<&str> {
        self.get_feature("checkpointing")
            .and_then(|f| f.backends())
            .map(|b| b.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Get all streaming backends
    #[must_use]
    pub fn streaming_backends(&self) -> Vec<&str> {
        self.get_feature("streaming")
            .and_then(|f| f.backends())
            .map(|b| b.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Get optimization algorithms
    #[must_use]
    pub fn optimization_algorithms(&self) -> Vec<&str> {
        self.get_feature("optimization")
            .and_then(|f| f.algorithms())
            .map(|a| a.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Check if a specific LLM provider is supported
    #[must_use]
    pub fn supports_llm_provider(&self, provider: &str) -> bool {
        self.get_feature("llm_providers")
            .map(|f| f.supports(provider))
            .unwrap_or(false)
    }

    /// Check if a specific vector store is supported
    #[must_use]
    pub fn supports_vector_store(&self, store: &str) -> bool {
        self.get_feature("vector_stores")
            .map(|f| f.supports(store))
            .unwrap_or(false)
    }

    /// Check if a checkpoint backend is supported
    #[must_use]
    pub fn supports_checkpoint_backend(&self, backend: &str) -> bool {
        self.get_feature("checkpointing")
            .map(|f| f.has_backend(backend))
            .unwrap_or(false)
    }

    /// Search features by name or description
    #[must_use]
    pub fn search_features(&self, query: &str) -> Vec<&FeatureInfo> {
        let query_lower = query.to_lowercase();
        self.features
            .iter()
            .filter(|f| {
                f.name.to_lowercase().contains(&query_lower)
                    || f.title.to_lowercase().contains(&query_lower)
                    || f.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// Get crates by category
    #[must_use]
    pub fn crates_by_category(&self, category: CrateCategory) -> Vec<&CrateInfo> {
        self.crates
            .iter()
            .filter(|c| c.category == category)
            .collect()
    }

    /// Get all LLM provider crates
    #[must_use]
    pub fn llm_providers(&self) -> Vec<&CrateInfo> {
        self.crates_by_category(CrateCategory::LlmProvider)
    }

    /// Get all vector store crates
    #[must_use]
    pub fn vector_stores(&self) -> Vec<&CrateInfo> {
        self.crates_by_category(CrateCategory::VectorStore)
    }

    /// Get all tool crates
    #[must_use]
    pub fn tools(&self) -> Vec<&CrateInfo> {
        self.crates_by_category(CrateCategory::Tool)
    }

    /// Get total API count
    #[must_use]
    pub fn api_count(&self) -> usize {
        self.modules.iter().map(|m| m.apis.len()).sum()
    }
}

/// Builder for PlatformRegistry
#[derive(Debug, Default)]
pub struct PlatformRegistryBuilder {
    version: Option<String>,
    modules: Vec<ModuleInfo>,
    features: Vec<FeatureInfo>,
    crates: Vec<CrateInfo>,
    metadata: Option<PlatformMetadata>,
}

impl PlatformRegistryBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the platform version
    #[must_use]
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Add a module
    pub fn add_module(&mut self, module: ModuleInfo) -> &mut Self {
        self.modules.push(module);
        self
    }

    /// Add a feature
    pub fn add_feature(&mut self, feature: FeatureInfo) -> &mut Self {
        self.features.push(feature);
        self
    }

    /// Add a crate
    pub fn add_crate(&mut self, crate_info: CrateInfo) -> &mut Self {
        self.crates.push(crate_info);
        self
    }

    /// Set metadata
    #[must_use]
    pub fn metadata(mut self, metadata: PlatformMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Build the registry
    #[must_use]
    pub fn build(self) -> PlatformRegistry {
        PlatformRegistry {
            version: self
                .version
                .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
            modules: self.modules,
            features: self.features,
            crates: self.crates,
            metadata: self.metadata.unwrap_or_default(),
        }
    }
}

/// Module information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    /// Module name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// APIs in this module
    pub apis: Vec<ApiInfo>,
}

impl ModuleInfo {
    /// Create a new module info builder
    #[must_use]
    pub fn builder() -> ModuleInfoBuilder {
        ModuleInfoBuilder::new()
    }
}

/// Builder for ModuleInfo
#[derive(Debug, Default)]
pub struct ModuleInfoBuilder {
    name: Option<String>,
    description: Option<String>,
    apis: Vec<ApiInfo>,
}

impl ModuleInfoBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the module name
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the description
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add an API
    #[must_use]
    pub fn add_api(mut self, api: ApiInfo) -> Self {
        self.apis.push(api);
        self
    }

    /// Build the module info
    #[must_use]
    pub fn build(self) -> ModuleInfo {
        ModuleInfo {
            name: self.name.unwrap_or_default(),
            description: self.description.unwrap_or_default(),
            apis: self.apis,
        }
    }
}

/// API information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiInfo {
    /// Function/type name
    pub function: String,
    /// Human-readable description
    pub description: String,
    /// Function signature
    pub signature: String,
    /// Usage example
    pub example: Option<String>,
}

impl ApiInfo {
    /// Create a new API info
    #[must_use]
    pub fn new(
        function: impl Into<String>,
        description: impl Into<String>,
        signature: impl Into<String>,
        example: Option<&str>,
    ) -> Self {
        Self {
            function: function.into(),
            description: description.into(),
            signature: signature.into(),
            example: example.map(String::from),
        }
    }
}

/// Feature information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureInfo {
    /// Feature name (identifier)
    pub name: String,
    /// Human-readable title
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Feature-specific details
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<FeatureDetails>,
}

impl FeatureInfo {
    /// Create a new feature info
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            title: title.into(),
            description: description.into(),
            details: None,
        }
    }

    /// Create a simple feature info using name as title (for platform_introspection compatibility)
    #[must_use]
    pub fn simple(name: impl Into<String>, description: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            title: name.clone(),
            name,
            description: description.into(),
            details: Some(FeatureDetails {
                enabled_by_default: true,
                ..Default::default()
            }),
        }
    }

    /// Create feature info with details
    #[must_use]
    pub fn with_details(
        name: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        details: FeatureDetails,
    ) -> Self {
        Self {
            name: name.into(),
            title: title.into(),
            description: description.into(),
            details: Some(details),
        }
    }

    /// Mark as disabled by default
    #[must_use]
    pub fn disabled_by_default(mut self) -> Self {
        self.details
            .get_or_insert_with(Default::default)
            .enabled_by_default = false;
        self
    }

    /// Add opt-out method
    #[must_use]
    pub fn with_opt_out(mut self, method: impl Into<String>) -> Self {
        self.details
            .get_or_insert_with(Default::default)
            .opt_out_method = Some(method.into());
        self
    }

    /// Add documentation URL
    #[must_use]
    pub fn with_docs(mut self, url: impl Into<String>) -> Self {
        self.details
            .get_or_insert_with(Default::default)
            .documentation_url = Some(url.into());
        self
    }

    /// Check if this feature is enabled by default
    #[must_use]
    pub fn default_enabled(&self) -> bool {
        self.details
            .as_ref()
            .map(|d| d.enabled_by_default)
            .unwrap_or(true)
    }

    /// Get the opt-out method if any
    #[must_use]
    pub fn opt_out_method(&self) -> Option<&str> {
        self.details
            .as_ref()
            .and_then(|d| d.opt_out_method.as_deref())
    }

    /// Get the documentation URL if any
    #[must_use]
    pub fn documentation_url(&self) -> Option<&str> {
        self.details
            .as_ref()
            .and_then(|d| d.documentation_url.as_deref())
    }

    /// Get backends if this feature has them
    #[must_use]
    pub fn backends(&self) -> Option<&[String]> {
        self.details.as_ref().and_then(|d| d.backends.as_deref())
    }

    /// Get supported items (providers, stores, etc.)
    #[must_use]
    pub fn supported(&self) -> Option<&[String]> {
        self.details.as_ref().and_then(|d| d.supported.as_deref())
    }

    /// Get algorithms if applicable
    #[must_use]
    pub fn algorithms(&self) -> Option<&[String]> {
        self.details.as_ref().and_then(|d| d.algorithms.as_deref())
    }

    /// Check if a specific backend is supported
    #[must_use]
    pub fn has_backend(&self, backend: &str) -> bool {
        self.backends()
            .map(|b| b.iter().any(|s| s.eq_ignore_ascii_case(backend)))
            .unwrap_or(false)
    }

    /// Check if a specific item is supported
    #[must_use]
    pub fn supports(&self, item: &str) -> bool {
        self.supported()
            .map(|s| s.iter().any(|i| i.eq_ignore_ascii_case(item)))
            .unwrap_or(false)
    }
}

/// Feature-specific details
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureDetails {
    /// Available backends (e.g., for checkpointing: Memory, SQLite, Redis)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backends: Option<Vec<String>>,
    /// Supported integrations (e.g., for LLM: OpenAI, Anthropic, Bedrock)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported: Option<Vec<String>>,
    /// Available algorithms (e.g., for optimization: MIPRO, DashOptimize)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub algorithms: Option<Vec<String>>,
    /// Configuration options
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_options: Option<Vec<ConfigOption>>,
    /// Whether this feature is enabled by default
    #[serde(default)]
    pub enabled_by_default: bool,
    /// Required dependencies
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Vec<String>>,
    /// Method to opt-out of this feature (if applicable)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opt_out_method: Option<String>,
    /// Link to documentation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
}

impl FeatureDetails {
    /// Create a new feature details builder
    #[must_use]
    pub fn builder() -> FeatureDetailsBuilder {
        FeatureDetailsBuilder::new()
    }
}

/// Builder for FeatureDetails
#[derive(Debug, Default)]
pub struct FeatureDetailsBuilder {
    backends: Option<Vec<String>>,
    supported: Option<Vec<String>>,
    algorithms: Option<Vec<String>>,
    config_options: Option<Vec<ConfigOption>>,
    enabled_by_default: bool,
    dependencies: Option<Vec<String>>,
    opt_out_method: Option<String>,
    documentation_url: Option<String>,
}

impl FeatureDetailsBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set available backends
    #[must_use]
    pub fn backends(mut self, backends: Vec<impl Into<String>>) -> Self {
        self.backends = Some(backends.into_iter().map(Into::into).collect());
        self
    }

    /// Set supported integrations
    #[must_use]
    pub fn supported(mut self, supported: Vec<impl Into<String>>) -> Self {
        self.supported = Some(supported.into_iter().map(Into::into).collect());
        self
    }

    /// Set available algorithms
    #[must_use]
    pub fn algorithms(mut self, algorithms: Vec<impl Into<String>>) -> Self {
        self.algorithms = Some(algorithms.into_iter().map(Into::into).collect());
        self
    }

    /// Add a configuration option
    #[must_use]
    pub fn config_option(mut self, option: ConfigOption) -> Self {
        self.config_options
            .get_or_insert_with(Vec::new)
            .push(option);
        self
    }

    /// Set whether enabled by default
    #[must_use]
    pub fn enabled_by_default(mut self, enabled: bool) -> Self {
        self.enabled_by_default = enabled;
        self
    }

    /// Set required dependencies
    #[must_use]
    pub fn dependencies(mut self, deps: Vec<impl Into<String>>) -> Self {
        self.dependencies = Some(deps.into_iter().map(Into::into).collect());
        self
    }

    /// Set method to opt-out of this feature
    #[must_use]
    pub fn opt_out_method(mut self, method: impl Into<String>) -> Self {
        self.opt_out_method = Some(method.into());
        self
    }

    /// Set documentation URL
    #[must_use]
    pub fn documentation_url(mut self, url: impl Into<String>) -> Self {
        self.documentation_url = Some(url.into());
        self
    }

    /// Build the feature details
    #[must_use]
    pub fn build(self) -> FeatureDetails {
        FeatureDetails {
            backends: self.backends,
            supported: self.supported,
            algorithms: self.algorithms,
            config_options: self.config_options,
            enabled_by_default: self.enabled_by_default,
            dependencies: self.dependencies,
            opt_out_method: self.opt_out_method,
            documentation_url: self.documentation_url,
        }
    }
}

/// Configuration option for a feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigOption {
    /// Option name
    pub name: String,
    /// Description
    pub description: String,
    /// Type (string, number, boolean, etc.)
    pub option_type: String,
    /// Default value (as string)
    pub default: Option<String>,
    /// Whether this option is required
    pub required: bool,
}

impl ConfigOption {
    /// Create a new configuration option
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        option_type: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            option_type: option_type.into(),
            default: None,
            required: false,
        }
    }

    /// Set default value
    #[must_use]
    pub fn with_default(mut self, default: impl Into<String>) -> Self {
        self.default = Some(default.into());
        self
    }

    /// Mark as required
    #[must_use]
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

// ============================================================================
// Documentation Querying
// ============================================================================

/// Documentation query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocResult {
    /// Title or topic of the result
    pub title: String,
    /// Content/description
    pub content: String,
    /// Relevance score (0.0 to 1.0)
    pub relevance: f64,
    /// Source (module name, function name, etc.)
    pub source: String,
    /// Code example if available
    pub example: Option<String>,
}

impl DocResult {
    /// Create a new doc result
    #[must_use]
    pub fn new(
        title: impl Into<String>,
        content: impl Into<String>,
        relevance: f64,
        source: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            relevance,
            source: source.into(),
            example: None,
        }
    }

    /// Add an example to the result
    #[must_use]
    pub fn with_example(mut self, example: impl Into<String>) -> Self {
        self.example = Some(example.into());
        self
    }
}

/// API documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiDocs {
    /// Function/type name
    pub name: String,
    /// Description
    pub description: String,
    /// Function signature
    pub signature: String,
    /// Parameters with descriptions
    pub parameters: Vec<ParamDoc>,
    /// Return type description
    pub returns: Option<String>,
    /// Usage examples
    pub examples: Vec<String>,
    /// Related APIs
    pub related: Vec<String>,
}

impl ApiDocs {
    /// Create a new API docs entry
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        signature: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            signature: signature.into(),
            parameters: Vec::new(),
            returns: None,
            examples: Vec::new(),
            related: Vec::new(),
        }
    }

    /// Add a parameter
    #[must_use]
    pub fn add_param(mut self, name: impl Into<String>, description: impl Into<String>) -> Self {
        self.parameters.push(ParamDoc {
            name: name.into(),
            description: description.into(),
        });
        self
    }

    /// Set return description
    #[must_use]
    pub fn returns(mut self, description: impl Into<String>) -> Self {
        self.returns = Some(description.into());
        self
    }

    /// Add an example
    #[must_use]
    pub fn add_example(mut self, example: impl Into<String>) -> Self {
        self.examples.push(example.into());
        self
    }

    /// Add a related API
    #[must_use]
    pub fn add_related(mut self, related: impl Into<String>) -> Self {
        self.related.push(related.into());
        self
    }
}

/// Parameter documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDoc {
    /// Parameter name
    pub name: String,
    /// Description
    pub description: String,
}

/// Documentation query interface for AI platform awareness
///
/// Enables AIs to search and retrieve documentation about DashFlow APIs.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::platform_registry::DocumentationQuery;
///
/// let docs = DocumentationQuery::new();
///
/// // Search for documentation
/// let results = docs.search("add node");
/// for result in results {
///     println!("{}: {}", result.title, result.content);
/// }
///
/// // Get example for a topic
/// if let Some(example) = docs.get_example("StateGraph") {
///     println!("Example: {}", example);
/// }
///
/// // Get API docs
/// if let Some(api) = docs.get_api_docs("StateGraph::add_node") {
///     println!("Signature: {}", api.signature);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct DocumentationQuery {
    /// Reference to platform registry
    registry: PlatformRegistry,
    /// Embedded documentation entries
    docs: Vec<DocEntry>,
}

/// Internal documentation entry
#[derive(Debug, Clone)]
struct DocEntry {
    topic: String,
    content: String,
    keywords: Vec<String>,
    example: Option<String>,
    module: Option<String>,
}

impl Default for DocumentationQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentationQuery {
    /// Create a new documentation query interface
    #[must_use]
    pub fn new() -> Self {
        let registry = PlatformRegistry::discover();
        let docs = Self::build_docs(&registry);
        Self { registry, docs }
    }

    /// Create documentation query with an existing registry
    #[must_use]
    pub fn with_registry(registry: PlatformRegistry) -> Self {
        let docs = Self::build_docs(&registry);
        Self { registry, docs }
    }

    /// Build documentation entries from registry
    fn build_docs(registry: &PlatformRegistry) -> Vec<DocEntry> {
        let mut docs = Vec::new();

        // Add module documentation
        for module in &registry.modules {
            docs.push(DocEntry {
                topic: module.name.clone(),
                content: module.description.clone(),
                keywords: vec![module.name.clone(), "module".to_string()],
                example: None,
                module: Some(module.name.clone()),
            });

            // Add API documentation
            for api in &module.apis {
                let keywords: Vec<String> = api
                    .function
                    .split("::")
                    .map(|s| s.to_lowercase())
                    .chain(api.description.split_whitespace().map(|s| s.to_lowercase()))
                    .filter(|s| s.len() > 2)
                    .collect();

                docs.push(DocEntry {
                    topic: api.function.clone(),
                    content: api.description.clone(),
                    keywords,
                    example: api.example.clone(),
                    module: Some(module.name.clone()),
                });
            }
        }

        // Add feature documentation
        for feature in &registry.features {
            let mut keywords: Vec<String> = feature
                .name
                .split('_')
                .map(|s| s.to_lowercase())
                .chain(
                    feature
                        .description
                        .split_whitespace()
                        .map(|s| s.to_lowercase()),
                )
                .filter(|s| s.len() > 2)
                .collect();
            keywords.push("feature".to_string());

            docs.push(DocEntry {
                topic: feature.title.clone(),
                content: feature.description.clone(),
                keywords,
                example: None,
                module: None,
            });
        }

        // Add concept documentation
        docs.extend(Self::concept_docs());

        docs
    }

    /// Built-in concept documentation
    fn concept_docs() -> Vec<DocEntry> {
        vec![
            DocEntry {
                topic: "Getting Started".to_string(),
                content: "Create a StateGraph, add nodes and edges, compile and invoke."
                    .to_string(),
                keywords: vec![
                    "start".to_string(),
                    "begin".to_string(),
                    "create".to_string(),
                    "new".to_string(),
                    "tutorial".to_string(),
                ],
                example: Some(
                    r#"let mut graph = StateGraph::new();
graph.add_node("process", |state| async move { state });
graph.set_entry_point("process");
let app = graph.compile()?;
let result = app.invoke(initial_state).await?;"#
                        .to_string(),
                ),
                module: None,
            },
            DocEntry {
                topic: "Conditional Routing".to_string(),
                content: "Use add_conditional_edges to route based on state.".to_string(),
                keywords: vec![
                    "condition".to_string(),
                    "conditional".to_string(),
                    "routing".to_string(),
                    "route".to_string(),
                    "branch".to_string(),
                    "if".to_string(),
                ],
                example: Some(
                    r#"graph.add_conditional_edges(
    "router",
    |state| match state.next.as_str() {
        "process" => "process".to_string(),
        _ => END.to_string(),
    }
);"#
                    .to_string(),
                ),
                module: Some("core".to_string()),
            },
            DocEntry {
                topic: "Checkpointing".to_string(),
                content: "Save and restore graph execution state for persistence and recovery."
                    .to_string(),
                keywords: vec![
                    "checkpoint".to_string(),
                    "save".to_string(),
                    "restore".to_string(),
                    "persist".to_string(),
                    "state".to_string(),
                    "recovery".to_string(),
                ],
                example: Some(
                    r#"let checkpointer = MemoryCheckpointer::new();
let app = graph.compile_with_checkpointer(checkpointer)?;
let config = RunnableConfig::new().with_thread_id("thread-1");
let result = app.invoke_with_config(state, config).await?;"#
                        .to_string(),
                ),
                module: Some("checkpoint".to_string()),
            },
            DocEntry {
                topic: "Streaming".to_string(),
                content: "Stream execution events for real-time updates.".to_string(),
                keywords: vec![
                    "stream".to_string(),
                    "streaming".to_string(),
                    "events".to_string(),
                    "realtime".to_string(),
                    "real-time".to_string(),
                    "live".to_string(),
                ],
                example: Some(
                    r#"let mut stream = app.stream(state);
while let Some(event) = stream.next().await {
    match event {
        StreamEvent::NodeStart { node, .. } => println!("Starting: {}", node),
        StreamEvent::NodeEnd { node, .. } => println!("Finished: {}", node),
        _ => {}
    }
}"#
                    .to_string(),
                ),
                module: Some("streaming".to_string()),
            },
            DocEntry {
                topic: "Cycles and Loops".to_string(),
                content: "Create cycles in your graph for iterative processing.".to_string(),
                keywords: vec![
                    "cycle".to_string(),
                    "loop".to_string(),
                    "iterate".to_string(),
                    "iteration".to_string(),
                    "repeat".to_string(),
                    "recursive".to_string(),
                ],
                example: Some(
                    r#"graph.add_edge("process", "check");
graph.add_conditional_edges(
    "check",
    |state| if state.should_continue() {
        "process".to_string()  // Loop back
    } else {
        END.to_string()
    }
);"#
                    .to_string(),
                ),
                module: Some("core".to_string()),
            },
            DocEntry {
                topic: "Parallel Execution".to_string(),
                content: "Execute multiple nodes in parallel with fan-out/fan-in.".to_string(),
                keywords: vec![
                    "parallel".to_string(),
                    "concurrent".to_string(),
                    "fanout".to_string(),
                    "fan-out".to_string(),
                    "fanin".to_string(),
                    "fan-in".to_string(),
                ],
                example: Some(
                    r#"// Multiple edges from same source run in parallel
graph.add_edge("start", "task_a");
graph.add_edge("start", "task_b");
graph.add_edge("start", "task_c");
// Results are merged at the next node"#
                        .to_string(),
                ),
                module: Some("core".to_string()),
            },
        ]
    }

    /// Search documentation by query string
    ///
    /// Returns results sorted by relevance.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<DocResult> {
        let query_terms: Vec<String> = query
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .filter(|s| s.len() > 1)
            .collect();

        if query_terms.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<(DocResult, f64)> = Vec::new();

        for doc in &self.docs {
            let mut score = 0.0;

            // Check topic match (highest weight)
            let topic_lower = doc.topic.to_lowercase();
            for term in &query_terms {
                if topic_lower.contains(term) {
                    score += 3.0;
                }
            }

            // Check keyword match
            for term in &query_terms {
                if doc.keywords.iter().any(|k| k.contains(term)) {
                    score += 2.0;
                }
            }

            // Check content match
            let content_lower = doc.content.to_lowercase();
            for term in &query_terms {
                if content_lower.contains(term) {
                    score += 1.0;
                }
            }

            if score > 0.0 {
                // Normalize score
                let max_score = (query_terms.len() * 6) as f64;
                let relevance = (score / max_score).min(1.0);

                let mut result = DocResult::new(
                    &doc.topic,
                    &doc.content,
                    relevance,
                    doc.module.as_deref().unwrap_or("general"),
                );

                if let Some(example) = &doc.example {
                    result = result.with_example(example);
                }

                results.push((result, score));
            }
        }

        // Sort by score descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        results.into_iter().map(|(r, _)| r).collect()
    }

    /// Get a code example for a topic
    #[must_use]
    pub fn get_example(&self, topic: &str) -> Option<String> {
        let topic_lower = topic.to_lowercase();

        // First check exact API matches
        for module in &self.registry.modules {
            for api in &module.apis {
                if api.function.to_lowercase().contains(&topic_lower) {
                    if let Some(example) = &api.example {
                        return Some(example.clone());
                    }
                }
            }
        }

        // Then check concept docs
        for doc in &self.docs {
            if doc.topic.to_lowercase().contains(&topic_lower)
                || doc.keywords.iter().any(|k| k.contains(&topic_lower))
            {
                if let Some(example) = &doc.example {
                    return Some(example.clone());
                }
            }
        }

        None
    }

    /// Get API documentation for a function
    #[must_use]
    pub fn get_api_docs(&self, function: &str) -> Option<ApiDocs> {
        let function_lower = function.to_lowercase();

        for module in &self.registry.modules {
            for api in &module.apis {
                if api.function.to_lowercase().contains(&function_lower) {
                    let mut docs = ApiDocs::new(&api.function, &api.description, &api.signature);

                    if let Some(example) = &api.example {
                        docs = docs.add_example(example);
                    }

                    // Find related APIs in same module
                    for other_api in &module.apis {
                        if other_api.function != api.function {
                            // Check if names share a common prefix (same struct)
                            let api_parts: Vec<&str> = api.function.split("::").collect();
                            let other_parts: Vec<&str> = other_api.function.split("::").collect();
                            if !api_parts.is_empty()
                                && !other_parts.is_empty()
                                && api_parts[0] == other_parts[0]
                            {
                                docs = docs.add_related(&other_api.function);
                            }
                        }
                    }

                    return Some(docs);
                }
            }
        }

        None
    }

    /// List all documented topics
    #[must_use]
    pub fn list_topics(&self) -> Vec<&str> {
        self.docs.iter().map(|d| d.topic.as_str()).collect()
    }

    /// Get all examples
    #[must_use]
    pub fn all_examples(&self) -> Vec<(&str, &str)> {
        self.docs
            .iter()
            .filter_map(|d| d.example.as_ref().map(|e| (d.topic.as_str(), e.as_str())))
            .collect()
    }

    /// Get topics by module
    #[must_use]
    pub fn topics_in_module(&self, module: &str) -> Vec<&str> {
        self.docs
            .iter()
            .filter(|d| d.module.as_deref() == Some(module))
            .map(|d| d.topic.as_str())
            .collect()
    }
}

// ============================================================================
// App Architecture Analysis
// ============================================================================

/// Application architecture analysis result
///
/// Provides AI agents with knowledge about how their application is built:
/// - What DashFlow features does this app use?
/// - What is the graph structure?
/// - What custom code exists?
/// - What dependencies are used?
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::platform_registry::AppArchitecture;
///
/// let app = graph.compile()?;
/// let arch = app.analyze_architecture();
///
/// // AI asks: "What DashFlow features am I using?"
/// for feature in &arch.dashflow_features_used {
///     println!("Using: {}", feature);
/// }
///
/// // AI asks: "What's my graph structure?"
/// println!("Nodes: {}", arch.graph_structure.nodes.len());
/// println!("Entry point: {}", arch.graph_structure.entry_point);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppArchitecture {
    /// Graph structure (from introspection manifest)
    pub graph_structure: ArchitectureGraphInfo,
    /// DashFlow features used by this application
    pub dashflow_features_used: Vec<FeatureUsage>,
    /// Custom code modules in the application
    pub custom_code: Vec<CodeModule>,
    /// Dependencies used by the application
    pub dependencies: Vec<Dependency>,
    /// Architecture metadata
    pub metadata: ArchitectureMetadata,
}

impl AppArchitecture {
    /// Create a new app architecture builder
    #[must_use]
    pub fn builder() -> AppArchitectureBuilder {
        AppArchitectureBuilder::new()
    }

    /// Convert architecture to JSON string for AI consumption
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Get a summary of the architecture
    #[must_use]
    pub fn summary(&self) -> String {
        let feature_count = self.dashflow_features_used.len();
        let node_count = self.graph_structure.node_count;
        let edge_count = self.graph_structure.edge_count;
        let custom_module_count = self.custom_code.len();
        let dependency_count = self.dependencies.len();

        format!(
            "App Architecture: {} nodes, {} edges, {} DashFlow features, {} custom modules, {} dependencies",
            node_count, edge_count, feature_count, custom_module_count, dependency_count
        )
    }

    /// Get features by category
    #[must_use]
    pub fn features_by_category(&self, category: &str) -> Vec<&FeatureUsage> {
        self.dashflow_features_used
            .iter()
            .filter(|f| f.category.eq_ignore_ascii_case(category))
            .collect()
    }

    /// Check if a specific feature is used
    #[must_use]
    pub fn uses_feature(&self, feature: &str) -> bool {
        self.dashflow_features_used
            .iter()
            .any(|f| f.name.eq_ignore_ascii_case(feature))
    }

    /// Get total lines of custom code
    #[must_use]
    pub fn total_custom_lines(&self) -> usize {
        self.custom_code.iter().map(|m| m.lines).sum()
    }

    /// Get DashFlow dependencies
    #[must_use]
    pub fn dashflow_dependencies(&self) -> Vec<&Dependency> {
        self.dependencies.iter().filter(|d| d.is_dashflow).collect()
    }

    /// Get external dependencies
    #[must_use]
    pub fn external_dependencies(&self) -> Vec<&Dependency> {
        self.dependencies
            .iter()
            .filter(|d| !d.is_dashflow)
            .collect()
    }
}

/// Builder for AppArchitecture
#[derive(Debug, Default)]
pub struct AppArchitectureBuilder {
    graph_structure: Option<ArchitectureGraphInfo>,
    dashflow_features_used: Vec<FeatureUsage>,
    custom_code: Vec<CodeModule>,
    dependencies: Vec<Dependency>,
    metadata: Option<ArchitectureMetadata>,
}

impl AppArchitectureBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set graph structure info
    #[must_use]
    pub fn graph_structure(mut self, info: ArchitectureGraphInfo) -> Self {
        self.graph_structure = Some(info);
        self
    }

    /// Add a feature usage
    pub fn add_feature(&mut self, feature: FeatureUsage) -> &mut Self {
        self.dashflow_features_used.push(feature);
        self
    }

    /// Add a custom code module
    pub fn add_code_module(&mut self, module: CodeModule) -> &mut Self {
        self.custom_code.push(module);
        self
    }

    /// Add a dependency
    pub fn add_dependency(&mut self, dep: Dependency) -> &mut Self {
        self.dependencies.push(dep);
        self
    }

    /// Set metadata
    #[must_use]
    pub fn metadata(mut self, metadata: ArchitectureMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Build the architecture
    #[must_use]
    pub fn build(self) -> AppArchitecture {
        AppArchitecture {
            graph_structure: self.graph_structure.unwrap_or_default(),
            dashflow_features_used: self.dashflow_features_used,
            custom_code: self.custom_code,
            dependencies: self.dependencies,
            metadata: self.metadata.unwrap_or_default(),
        }
    }
}

/// Simplified graph structure information for architecture analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArchitectureGraphInfo {
    /// Graph name (if set)
    pub name: Option<String>,
    /// Entry point node name
    pub entry_point: String,
    /// Number of nodes
    pub node_count: usize,
    /// Number of edges
    pub edge_count: usize,
    /// Node names
    pub node_names: Vec<String>,
    /// Whether the graph has cycles
    pub has_cycles: bool,
    /// Whether the graph has conditional edges
    pub has_conditional_edges: bool,
    /// Whether the graph has parallel edges
    pub has_parallel_edges: bool,
}

impl ArchitectureGraphInfo {
    /// Create new graph info
    #[must_use]
    pub fn new(entry_point: impl Into<String>) -> Self {
        Self {
            name: None,
            entry_point: entry_point.into(),
            node_count: 0,
            edge_count: 0,
            node_names: Vec::new(),
            has_cycles: false,
            has_conditional_edges: false,
            has_parallel_edges: false,
        }
    }

    /// Set graph name
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set node count
    #[must_use]
    pub fn with_node_count(mut self, count: usize) -> Self {
        self.node_count = count;
        self
    }

    /// Set edge count
    #[must_use]
    pub fn with_edge_count(mut self, count: usize) -> Self {
        self.edge_count = count;
        self
    }

    /// Set node names
    #[must_use]
    pub fn with_node_names(mut self, names: Vec<String>) -> Self {
        self.node_names = names;
        self
    }

    /// Set has cycles
    #[must_use]
    pub fn with_cycles(mut self, has_cycles: bool) -> Self {
        self.has_cycles = has_cycles;
        self
    }

    /// Set has conditional edges
    #[must_use]
    pub fn with_conditional_edges(mut self, has: bool) -> Self {
        self.has_conditional_edges = has;
        self
    }

    /// Set has parallel edges
    #[must_use]
    pub fn with_parallel_edges(mut self, has: bool) -> Self {
        self.has_parallel_edges = has;
        self
    }
}

/// Feature usage information
///
/// Describes how a DashFlow feature is used in the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureUsage {
    /// Feature name
    pub name: String,
    /// Feature category (e.g., "core", "checkpoint", "llm")
    pub category: String,
    /// Description of how the feature is used
    pub description: String,
    /// APIs used from this feature
    pub apis_used: Vec<String>,
    /// Whether this is a core/required feature
    pub is_core: bool,
}

impl FeatureUsage {
    /// Create a new feature usage
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        category: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            category: category.into(),
            description: description.into(),
            apis_used: Vec::new(),
            is_core: false,
        }
    }

    /// Add an API used
    #[must_use]
    pub fn with_api(mut self, api: impl Into<String>) -> Self {
        self.apis_used.push(api.into());
        self
    }

    /// Add multiple APIs
    #[must_use]
    pub fn with_apis(mut self, apis: Vec<impl Into<String>>) -> Self {
        self.apis_used.extend(apis.into_iter().map(Into::into));
        self
    }

    /// Mark as core feature
    #[must_use]
    pub fn core(mut self) -> Self {
        self.is_core = true;
        self
    }
}

/// Custom code module information
///
/// Describes a custom code module in the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeModule {
    /// Module name
    pub name: String,
    /// Source file path (if known)
    pub file: Option<String>,
    /// Number of lines of code
    pub lines: usize,
    /// DashFlow APIs used by this module
    pub dashflow_apis_used: Vec<String>,
    /// Module description/purpose
    pub description: Option<String>,
}

impl CodeModule {
    /// Create a new code module
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            file: None,
            lines: 0,
            dashflow_apis_used: Vec::new(),
            description: None,
        }
    }

    /// Set source file
    #[must_use]
    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// Set line count
    #[must_use]
    pub fn with_lines(mut self, lines: usize) -> Self {
        self.lines = lines;
        self
    }

    /// Add a DashFlow API used
    #[must_use]
    pub fn with_api(mut self, api: impl Into<String>) -> Self {
        self.dashflow_apis_used.push(api.into());
        self
    }

    /// Add multiple APIs
    #[must_use]
    pub fn with_apis(mut self, apis: Vec<impl Into<String>>) -> Self {
        self.dashflow_apis_used
            .extend(apis.into_iter().map(Into::into));
        self
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Dependency information
///
/// Describes a dependency used by the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Crate name
    pub name: String,
    /// Version (if known)
    pub version: Option<String>,
    /// Purpose/description
    pub purpose: String,
    /// Whether this is a DashFlow crate
    pub is_dashflow: bool,
    /// APIs used from this dependency
    pub apis_used: Vec<String>,
}

impl Dependency {
    /// Create a new dependency
    #[must_use]
    pub fn new(name: impl Into<String>, purpose: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: None,
            purpose: purpose.into(),
            is_dashflow: false,
            apis_used: Vec::new(),
        }
    }

    /// Set version
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Mark as DashFlow crate
    #[must_use]
    pub fn dashflow(mut self) -> Self {
        self.is_dashflow = true;
        self
    }

    /// Add an API used
    #[must_use]
    pub fn with_api(mut self, api: impl Into<String>) -> Self {
        self.apis_used.push(api.into());
        self
    }
}

/// Architecture metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArchitectureMetadata {
    /// DashFlow version
    pub dashflow_version: String,
    /// When the architecture was analyzed
    pub analyzed_at: Option<String>,
    /// Analysis notes
    pub notes: Vec<String>,
}

impl ArchitectureMetadata {
    /// Create new metadata with DashFlow version
    #[must_use]
    pub fn new() -> Self {
        Self {
            dashflow_version: env!("CARGO_PKG_VERSION").to_string(),
            analyzed_at: None,
            notes: Vec::new(),
        }
    }

    /// Add a note
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// Crate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateInfo {
    /// Crate name
    pub name: String,
    /// Description
    pub description: String,
    /// Category
    pub category: CrateCategory,
}

impl CrateInfo {
    /// Create a new crate info
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        category: CrateCategory,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            category,
        }
    }
}

/// Crate category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrateCategory {
    /// Core framework
    Core,
    /// LLM provider integration
    LlmProvider,
    /// Vector store integration
    VectorStore,
    /// Tool for agents
    Tool,
    /// Checkpoint backend
    Checkpointer,
    /// Embedding provider
    Embedding,
    /// Search integration
    Search,
    /// Other integration
    Other,
}

impl std::fmt::Display for CrateCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core => write!(f, "Core"),
            Self::LlmProvider => write!(f, "LLM Provider"),
            Self::VectorStore => write!(f, "Vector Store"),
            Self::Tool => write!(f, "Tool"),
            Self::Checkpointer => write!(f, "Checkpointer"),
            Self::Embedding => write!(f, "Embedding"),
            Self::Search => write!(f, "Search"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Platform metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformMetadata {
    /// Platform name
    pub name: String,
    /// Repository URL
    pub repository: Option<String>,
    /// Documentation URL
    pub documentation: Option<String>,
    /// License
    pub license: Option<String>,
}

impl Default for PlatformMetadata {
    fn default() -> Self {
        Self {
            name: "DashFlow".to_string(),
            repository: Some("https://github.com/dropbox/dTOOL/dashflow".to_string()),
            documentation: Some("https://docs.dashflow.dev".to_string()),
            license: Some("MIT".to_string()),
        }
    }
}

impl PlatformMetadata {
    /// Create new platform metadata
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

// ============================================================================
// Canonical Type Definitions (M-603)
// ============================================================================
// These define the canonical conceptual types for nodes, edges, templates, and states.
// PlatformIntrospection delegates to these to establish a single source of truth.

use crate::platform_introspection::{EdgeTypeInfo, NodeTypeInfo, StateTypeInfo, TemplateInfo};

/// Returns the canonical list of supported node types.
///
/// These are conceptual patterns for how nodes can be used in graphs,
/// not an exhaustive list of all Node trait implementations.
#[must_use]
pub fn canonical_node_types() -> Vec<NodeTypeInfo> {
    vec![
        NodeTypeInfo::new("function", "A pure function that transforms state")
            .with_example("graph.add_node(\"process\", |state| async move { Ok(state) });"),
        NodeTypeInfo::new("agent", "An LLM-powered agent with tools")
            .with_example("graph.add_node(\"agent\", agent_node);"),
        NodeTypeInfo::new("tool", "A tool that performs a specific action")
            .with_example("graph.add_node(\"search\", tool_node);"),
        NodeTypeInfo::new("subgraph", "A nested graph for composition")
            .with_example("graph.add_node(\"sub\", SubgraphNode::new(inner_graph));"),
        NodeTypeInfo::new("conditional", "A routing node with conditional edges"),
        NodeTypeInfo::new("parallel", "A node that spawns parallel execution"),
        NodeTypeInfo::new("approval", "A human-in-the-loop approval node")
            .with_example("graph.add_node(\"approve\", approval_node);"),
    ]
}

/// Returns the canonical list of supported edge types.
///
/// These are the patterns for connecting nodes in a graph.
#[must_use]
pub fn canonical_edge_types() -> Vec<EdgeTypeInfo> {
    vec![
        EdgeTypeInfo::new("simple", "Direct connection between two nodes")
            .with_example("graph.add_edge(\"a\", \"b\");"),
        EdgeTypeInfo::new("conditional", "Dynamic routing based on state")
            .with_example("graph.add_conditional_edges(\"router\", |s| s.next.clone());"),
        EdgeTypeInfo::new("parallel", "Fork execution to multiple nodes")
            .with_example("graph.add_parallel_edges(\"start\", &[\"a\", \"b\", \"c\"]);"),
        EdgeTypeInfo::new("to_end", "Connection to the END node")
            .with_example("graph.add_edge(\"final\", END);"),
    ]
}

/// Returns the canonical list of built-in templates.
///
/// These are pre-built graph patterns for common use cases.
#[must_use]
pub fn canonical_templates() -> Vec<TemplateInfo> {
    vec![
        TemplateInfo::new("supervisor", "Multi-agent supervisor pattern")
            .with_use_case("Coordinating multiple specialized agents")
            .with_use_case("Task delegation and result aggregation")
            .with_example("SupervisorBuilder::new().add_worker(\"researcher\", agent).build();"),
        TemplateInfo::new("react_agent", "ReAct (Reasoning + Acting) agent pattern")
            .with_use_case("Tool-using LLM agents")
            .with_use_case("Step-by-step reasoning with actions")
            .with_example("create_react_agent(model, tools);"),
        TemplateInfo::new("map_reduce", "Parallel processing with aggregation")
            .with_use_case("Processing large datasets in parallel")
            .with_use_case("Batch API calls with result merging")
            .with_example("MapReduceBuilder::new().map_fn(process).reduce_fn(merge).build();"),
    ]
}

/// Returns the canonical list of available state types.
///
/// These are the MergeableState implementations available for use.
#[must_use]
pub fn canonical_state_types() -> Vec<StateTypeInfo> {
    vec![
        StateTypeInfo::new(
            "JsonState",
            "Generic JSON-based state with automatic merging",
        ),
        StateTypeInfo::new(
            "AgentState",
            "Pre-built state for agent workflows with messages",
        ),
        StateTypeInfo::new(
            "Custom",
            "User-defined state implementing MergeableState trait",
        )
        .custom(),
    ]
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests;
