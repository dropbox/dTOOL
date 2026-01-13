use super::dependency_analysis::{extract_features, extract_version, infer_purpose};
use super::*;

// -------------------------------------------------------------------------
// Platform API Registry Tests
// -------------------------------------------------------------------------

#[test]
fn test_platform_registry_discover() {
    let registry = PlatformRegistry::discover();

    // Should have modules
    assert!(!registry.modules.is_empty());

    // Should have features
    assert!(!registry.features.is_empty());

    // Should have crates
    assert!(!registry.crates.is_empty());

    // Version should be set
    assert!(!registry.version.is_empty());
}

#[test]
fn test_platform_registry_modules() {
    let registry = PlatformRegistry::discover();

    // Should have core module
    assert!(registry.modules.iter().any(|m| m.name == "core"));

    // Should have introspection module
    assert!(registry.modules.iter().any(|m| m.name == "introspection"));

    // Module names should be queryable
    let names = registry.module_names();
    assert!(names.contains(&"core"));
}

#[test]
fn test_platform_registry_find_api() {
    let registry = PlatformRegistry::discover();

    // Should find StateGraph::new
    let api = registry.find_api("StateGraph::new");
    assert!(api.is_some());
    assert!(api.unwrap().description.contains("state graph"));

    // Should find by partial match
    let api = registry.find_api("compile");
    assert!(api.is_some());

    // Case insensitive
    let api = registry.find_api("STATEGRAPH");
    assert!(api.is_some());
}

#[test]
fn test_platform_registry_search_apis() {
    let registry = PlatformRegistry::discover();

    // Should find multiple graph-related APIs
    let results = registry.search_apis("graph");
    assert!(!results.is_empty());

    // Should find APIs by description
    let results = registry.search_apis("execution");
    assert!(!results.is_empty());
}

#[test]
fn test_platform_registry_features() {
    let registry = PlatformRegistry::discover();

    // Should have key features
    assert!(registry.has_feature("graph_orchestration"));
    assert!(registry.has_feature("checkpointing"));
    assert!(registry.has_feature("streaming"));
    assert!(registry.has_feature("introspection"));

    // Feature names should be queryable
    let names = registry.feature_names();
    assert!(names.contains(&"graph_orchestration"));
}

#[test]
fn test_platform_registry_crates() {
    let registry = PlatformRegistry::discover();

    // Should have LLM providers
    let llm_providers = registry.llm_providers();
    assert!(!llm_providers.is_empty());
    assert!(llm_providers.iter().any(|c| c.name == "dashflow-openai"));

    // Should have vector stores
    let vector_stores = registry.vector_stores();
    assert!(!vector_stores.is_empty());

    // Should have tools
    let tools = registry.tools();
    assert!(!tools.is_empty());
}

#[test]
fn test_platform_registry_json_serialization() {
    let registry = PlatformRegistry::discover();

    // Should serialize to JSON
    let json = registry.to_json().unwrap();
    assert!(!json.is_empty());
    assert!(json.contains("dashflow"));

    // Should parse back
    let parsed = PlatformRegistry::from_json(&json).unwrap();
    assert_eq!(parsed.version, registry.version);
    assert_eq!(parsed.modules.len(), registry.modules.len());
}

#[test]
fn test_platform_registry_compact_json() {
    let registry = PlatformRegistry::discover();

    let pretty = registry.to_json().unwrap();
    let compact = registry.to_json_compact().unwrap();

    // Compact should be smaller
    assert!(compact.len() < pretty.len());

    // Both should parse
    let _ = PlatformRegistry::from_json(&compact).unwrap();
}

#[test]
fn test_platform_registry_api_count() {
    let registry = PlatformRegistry::discover();

    let count = registry.api_count();
    assert!(count > 0);

    // Should match sum of module APIs
    let manual_count: usize = registry.modules.iter().map(|m| m.apis.len()).sum();
    assert_eq!(count, manual_count);
}

#[test]
fn test_platform_registry_apis_in_module() {
    let registry = PlatformRegistry::discover();

    let core_apis = registry.apis_in_module("core");
    assert!(!core_apis.is_empty());

    // Non-existent module should return empty
    let empty = registry.apis_in_module("nonexistent");
    assert!(empty.is_empty());
}

#[test]
fn test_module_info_builder() {
    let module = ModuleInfo::builder()
        .name("test_module")
        .description("A test module")
        .add_api(ApiInfo::new(
            "test_fn",
            "A test function",
            "fn test()",
            None,
        ))
        .build();

    assert_eq!(module.name, "test_module");
    assert_eq!(module.description, "A test module");
    assert_eq!(module.apis.len(), 1);
}

#[test]
fn test_api_info_creation() {
    let api = ApiInfo::new(
        "example_fn",
        "An example function",
        "fn example() -> Result<()>",
        Some("let _ = example();"),
    );

    assert_eq!(api.function, "example_fn");
    assert_eq!(api.description, "An example function");
    assert!(api.example.is_some());
}

#[test]
fn test_feature_info_creation() {
    let feature = FeatureInfo::new("test_feature", "Test Feature", "A feature for testing");

    assert_eq!(feature.name, "test_feature");
    assert_eq!(feature.title, "Test Feature");
}

#[test]
fn test_crate_info_creation() {
    let crate_info = CrateInfo::new("dashflow-test", "A test crate", CrateCategory::Tool);

    assert_eq!(crate_info.name, "dashflow-test");
    assert_eq!(crate_info.category, CrateCategory::Tool);
}

#[test]
fn test_crate_category_display() {
    assert_eq!(format!("{}", CrateCategory::Core), "Core");
    assert_eq!(format!("{}", CrateCategory::LlmProvider), "LLM Provider");
    assert_eq!(format!("{}", CrateCategory::VectorStore), "Vector Store");
}

#[test]
fn test_platform_registry_builder() {
    let registry = PlatformRegistryBuilder::new().version("1.0.0").build();

    assert_eq!(registry.version, "1.0.0");
}

#[test]
fn test_platform_metadata() {
    let metadata = PlatformMetadata::new();

    assert_eq!(metadata.name, "DashFlow");
    assert!(metadata.repository.is_some());
    assert!(metadata.documentation.is_some());
}

#[test]
fn test_crates_by_category() {
    let registry = PlatformRegistry::discover();

    // Core should have at least dashflow
    let core = registry.crates_by_category(CrateCategory::Core);
    assert!(!core.is_empty());

    // All returned crates should match category
    for crate_info in core {
        assert_eq!(crate_info.category, CrateCategory::Core);
    }
}

#[test]
fn test_platform_registry_completeness() {
    let registry = PlatformRegistry::discover();

    // Should have comprehensive coverage
    assert!(
        registry.modules.len() >= 5,
        "Should have at least 5 modules"
    );
    assert!(
        registry.features.len() >= 5,
        "Should have at least 5 features"
    );
    assert!(
        registry.crates.len() >= 10,
        "Should have at least 10 crates"
    );
    assert!(registry.api_count() >= 10, "Should have at least 10 APIs");
}

#[test]
fn test_api_examples_present() {
    let registry = PlatformRegistry::discover();

    // Most APIs should have examples
    let apis_with_examples = registry
        .modules
        .iter()
        .flat_map(|m| &m.apis)
        .filter(|a| a.example.is_some())
        .count();

    let total_apis = registry.api_count();
    let ratio = apis_with_examples as f64 / total_apis as f64;

    assert!(ratio >= 0.5, "At least 50% of APIs should have examples");
}

#[test]
fn test_registry_default_version() {
    let registry = PlatformRegistryBuilder::new().build();

    // Should use CARGO_PKG_VERSION
    assert!(!registry.version.is_empty());
}

// AI usage scenario tests

#[test]
fn test_ai_can_discover_platform() {
    // Scenario: AI asks "What is DashFlow?"
    let platform = PlatformRegistry::discover();

    // AI should see platform name
    assert_eq!(platform.metadata.name, "DashFlow");

    // AI should see features
    assert!(!platform.features.is_empty());

    // AI should see modules
    assert!(!platform.modules.is_empty());
}

#[test]
fn test_ai_can_find_how_to_create_graph() {
    // Scenario: AI asks "How do I create a graph?"
    let platform = PlatformRegistry::discover();

    let api = platform.find_api("StateGraph::new");
    assert!(api.is_some());

    let api = api.unwrap();
    assert!(api.example.is_some());
    assert!(api.example.as_ref().unwrap().contains("StateGraph::new"));
}

#[test]
fn test_ai_can_list_llm_providers() {
    // Scenario: AI asks "What LLM providers are available?"
    let platform = PlatformRegistry::discover();

    let providers = platform.llm_providers();
    assert!(!providers.is_empty());

    // Should have OpenAI
    assert!(providers.iter().any(|p| p.name.contains("openai")));
}

#[test]
fn test_ai_can_check_features() {
    // Scenario: AI asks "Can I use checkpointing?"
    let platform = PlatformRegistry::discover();

    assert!(platform.has_feature("checkpointing"));
}

#[test]
fn test_ai_can_search_for_functionality() {
    // Scenario: AI asks "How do I stream execution?"
    let platform = PlatformRegistry::discover();

    let results = platform.search_apis("stream");
    assert!(!results.is_empty());
}

// -------------------------------------------------------------------------
// Feature Catalog Tests
// -------------------------------------------------------------------------

#[test]
fn test_feature_details_builder() {
    let details = FeatureDetails::builder()
        .backends(vec!["Memory", "SQLite", "Redis"])
        .supported(vec!["OpenAI", "Anthropic"])
        .algorithms(vec!["MIPRO", "DashOptimize"])
        .enabled_by_default(true)
        .dependencies(vec!["tokio", "serde"])
        .build();

    assert_eq!(details.backends.as_ref().unwrap().len(), 3);
    assert_eq!(details.supported.as_ref().unwrap().len(), 2);
    assert_eq!(details.algorithms.as_ref().unwrap().len(), 2);
    assert!(details.enabled_by_default);
    assert_eq!(details.dependencies.as_ref().unwrap().len(), 2);
}

#[test]
fn test_feature_info_with_details() {
    let feature = FeatureInfo::with_details(
        "test_feature",
        "Test Feature",
        "A test feature with details",
        FeatureDetails::builder()
            .backends(vec!["Memory", "SQLite"])
            .enabled_by_default(true)
            .build(),
    );

    assert_eq!(feature.name, "test_feature");
    assert!(feature.details.is_some());
    assert!(feature.backends().is_some());
    assert_eq!(feature.backends().unwrap().len(), 2);
}

#[test]
fn test_feature_info_backends() {
    let feature = FeatureInfo::with_details(
        "checkpointing",
        "Checkpointing",
        "State persistence",
        FeatureDetails::builder()
            .backends(vec!["Memory", "SQLite", "Redis"])
            .build(),
    );

    let backends = feature.backends().unwrap();
    assert!(backends.contains(&"Memory".to_string()));
    assert!(backends.contains(&"SQLite".to_string()));
    assert!(backends.contains(&"Redis".to_string()));
}

#[test]
fn test_feature_info_has_backend() {
    let feature = FeatureInfo::with_details(
        "checkpointing",
        "Checkpointing",
        "State persistence",
        FeatureDetails::builder()
            .backends(vec!["Memory", "SQLite", "Redis"])
            .build(),
    );

    assert!(feature.has_backend("Memory"));
    assert!(feature.has_backend("memory")); // case insensitive
    assert!(feature.has_backend("SQLITE")); // case insensitive
    assert!(!feature.has_backend("PostgreSQL"));
}

#[test]
fn test_feature_info_supports() {
    let feature = FeatureInfo::with_details(
        "llm_providers",
        "LLM Providers",
        "LLM integrations",
        FeatureDetails::builder()
            .supported(vec!["OpenAI", "Anthropic", "Bedrock"])
            .build(),
    );

    assert!(feature.supports("OpenAI"));
    assert!(feature.supports("openai")); // case insensitive
    assert!(feature.supports("ANTHROPIC")); // case insensitive
    assert!(!feature.supports("Cohere"));
}

#[test]
fn test_feature_info_algorithms() {
    let feature = FeatureInfo::with_details(
        "optimization",
        "Optimization",
        "Prompt optimization",
        FeatureDetails::builder()
            .algorithms(vec!["MIPRO", "DashOptimize", "BootstrapFewShot"])
            .build(),
    );

    let algorithms = feature.algorithms().unwrap();
    assert_eq!(algorithms.len(), 3);
    assert!(algorithms.contains(&"MIPRO".to_string()));
}

#[test]
fn test_config_option() {
    let option = ConfigOption::new("timeout", "Request timeout in seconds", "number")
        .with_default("30")
        .required();

    assert_eq!(option.name, "timeout");
    assert_eq!(option.option_type, "number");
    assert_eq!(option.default, Some("30".to_string()));
    assert!(option.required);
}

#[test]
fn test_feature_details_with_config_options() {
    let details = FeatureDetails::builder()
        .config_option(ConfigOption::new("timeout", "Timeout", "number").with_default("30"))
        .config_option(ConfigOption::new("retries", "Retry count", "number").required())
        .build();

    let options = details.config_options.unwrap();
    assert_eq!(options.len(), 2);
    assert_eq!(options[0].name, "timeout");
    assert_eq!(options[1].name, "retries");
}

#[test]
fn test_registry_get_feature() {
    let registry = PlatformRegistry::discover();

    let feature = registry.get_feature("checkpointing");
    assert!(feature.is_some());
    assert_eq!(feature.unwrap().name, "checkpointing");

    let missing = registry.get_feature("nonexistent");
    assert!(missing.is_none());
}

#[test]
fn test_registry_default_features() {
    let registry = PlatformRegistry::discover();

    let defaults = registry.default_features();
    assert!(!defaults.is_empty());

    // All should be enabled by default
    for feature in defaults {
        assert!(feature
            .details
            .as_ref()
            .map(|d| d.enabled_by_default)
            .unwrap_or(false));
    }
}

#[test]
fn test_registry_features_with_backends() {
    let registry = PlatformRegistry::discover();

    let with_backends = registry.features_with_backends();
    assert!(!with_backends.is_empty());

    // All should have backends
    for feature in with_backends {
        assert!(feature.backends().is_some());
    }
}

#[test]
fn test_registry_supported_llm_providers() {
    let registry = PlatformRegistry::discover();

    let providers = registry.supported_llm_providers();
    assert!(!providers.is_empty());
    assert!(providers.contains(&"OpenAI"));
    assert!(providers.contains(&"Anthropic"));
}

#[test]
fn test_registry_supported_vector_stores() {
    let registry = PlatformRegistry::discover();

    let stores = registry.supported_vector_stores();
    assert!(!stores.is_empty());
    assert!(stores.contains(&"Chroma"));
    assert!(stores.contains(&"Pinecone"));
}

#[test]
fn test_registry_supported_tools() {
    let registry = PlatformRegistry::discover();

    let tools = registry.supported_tools();
    assert!(!tools.is_empty());
    assert!(tools.contains(&"Shell"));
    assert!(tools.contains(&"File"));
}

#[test]
fn test_registry_checkpoint_backends() {
    let registry = PlatformRegistry::discover();

    let backends = registry.checkpoint_backends();
    assert!(!backends.is_empty());
    assert!(backends.contains(&"Memory"));
    assert!(backends.contains(&"SQLite"));
    assert!(backends.contains(&"Redis"));
}

#[test]
fn test_registry_streaming_backends() {
    let registry = PlatformRegistry::discover();

    let backends = registry.streaming_backends();
    assert!(!backends.is_empty());
    assert!(backends.contains(&"WebSocket"));
    assert!(backends.contains(&"SSE"));
}

#[test]
fn test_registry_optimization_algorithms() {
    let registry = PlatformRegistry::discover();

    let algorithms = registry.optimization_algorithms();
    assert!(!algorithms.is_empty());
    assert!(algorithms.contains(&"MIPRO"));
    assert!(algorithms.contains(&"DashOptimize"));
}

#[test]
fn test_registry_supports_llm_provider() {
    let registry = PlatformRegistry::discover();

    assert!(registry.supports_llm_provider("OpenAI"));
    assert!(registry.supports_llm_provider("openai")); // case insensitive
    assert!(registry.supports_llm_provider("Anthropic"));
    assert!(!registry.supports_llm_provider("FakeProvider"));
}

#[test]
fn test_registry_supports_vector_store() {
    let registry = PlatformRegistry::discover();

    assert!(registry.supports_vector_store("Chroma"));
    assert!(registry.supports_vector_store("chroma")); // case insensitive
    assert!(registry.supports_vector_store("Pinecone"));
    assert!(!registry.supports_vector_store("FakeStore"));
}

#[test]
fn test_registry_supports_checkpoint_backend() {
    let registry = PlatformRegistry::discover();

    assert!(registry.supports_checkpoint_backend("Memory"));
    assert!(registry.supports_checkpoint_backend("memory")); // case insensitive
    assert!(registry.supports_checkpoint_backend("SQLite"));
    assert!(!registry.supports_checkpoint_backend("FakeBackend"));
}

#[test]
fn test_registry_search_features() {
    let registry = PlatformRegistry::discover();

    // Search by name
    let results = registry.search_features("checkpoint");
    assert!(!results.is_empty());
    assert!(results.iter().any(|f| f.name == "checkpointing"));

    // Search by description
    let results = registry.search_features("persistence");
    assert!(!results.is_empty());

    // Case insensitive
    let results = registry.search_features("STREAMING");
    assert!(!results.is_empty());
}

#[test]
fn test_feature_catalog_has_llm_providers_feature() {
    let registry = PlatformRegistry::discover();

    assert!(registry.has_feature("llm_providers"));
    let feature = registry.get_feature("llm_providers").unwrap();
    assert!(feature.supported().is_some());
}

#[test]
fn test_feature_catalog_has_vector_stores_feature() {
    let registry = PlatformRegistry::discover();

    assert!(registry.has_feature("vector_stores"));
    let feature = registry.get_feature("vector_stores").unwrap();
    assert!(feature.supported().is_some());
}

#[test]
fn test_feature_catalog_has_tools_feature() {
    let registry = PlatformRegistry::discover();

    assert!(registry.has_feature("tools"));
    let feature = registry.get_feature("tools").unwrap();
    assert!(feature.supported().is_some());
}

#[test]
fn test_feature_catalog_has_embeddings_feature() {
    let registry = PlatformRegistry::discover();

    assert!(registry.has_feature("embeddings"));
    let feature = registry.get_feature("embeddings").unwrap();
    assert!(feature.supported().is_some());
}

#[test]
fn test_feature_info_without_details_accessors() {
    let feature = FeatureInfo::new("simple", "Simple", "A simple feature");

    assert!(feature.backends().is_none());
    assert!(feature.supported().is_none());
    assert!(feature.algorithms().is_none());
    assert!(!feature.has_backend("anything"));
    assert!(!feature.supports("anything"));
}

#[test]
fn test_feature_details_json_serialization() {
    let feature = FeatureInfo::with_details(
        "test",
        "Test",
        "Description",
        FeatureDetails::builder()
            .backends(vec!["A", "B"])
            .supported(vec!["X", "Y"])
            .enabled_by_default(true)
            .build(),
    );

    let json = serde_json::to_string(&feature).unwrap();
    assert!(json.contains("backends"));
    assert!(json.contains("supported"));

    let parsed: FeatureInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.backends().unwrap().len(), 2);
}

#[test]
fn test_feature_details_empty_serialization() {
    let feature = FeatureInfo::new("simple", "Simple", "No details");

    let json = serde_json::to_string(&feature).unwrap();
    // Should not contain "details" key when None
    assert!(!json.contains("\"details\""));
}

// AI usage scenario tests

#[test]
fn test_ai_can_ask_what_llm_providers_available() {
    // Scenario: AI asks "What LLM providers can I use?"
    let platform = PlatformRegistry::discover();

    let providers = platform.supported_llm_providers();
    assert!(!providers.is_empty());

    // AI sees a list: ["OpenAI", "Anthropic", "AWS Bedrock", ...]
    assert!(providers.contains(&"OpenAI"));
}

#[test]
fn test_ai_can_ask_if_openai_supported() {
    // Scenario: AI asks "Can I use OpenAI?"
    let platform = PlatformRegistry::discover();

    assert!(platform.supports_llm_provider("OpenAI"));
}

#[test]
fn test_ai_can_ask_what_checkpoint_backends() {
    // Scenario: AI asks "What checkpoint backends are available?"
    let platform = PlatformRegistry::discover();

    let backends = platform.checkpoint_backends();
    assert!(!backends.is_empty());

    // AI sees: ["Memory", "SQLite", "Redis", ...]
    assert!(backends.contains(&"Memory"));
}

#[test]
fn test_ai_can_ask_if_redis_checkpoint_supported() {
    // Scenario: AI asks "Can I use Redis for checkpointing?"
    let platform = PlatformRegistry::discover();

    assert!(platform.supports_checkpoint_backend("Redis"));
}

#[test]
fn test_ai_can_ask_what_optimization_algorithms() {
    // Scenario: AI asks "What prompt optimization algorithms are available?"
    let platform = PlatformRegistry::discover();

    let algorithms = platform.optimization_algorithms();
    assert!(!algorithms.is_empty());

    // AI sees: ["MIPRO", "DashOptimize", ...]
    assert!(algorithms.contains(&"MIPRO"));
}

#[test]
fn test_ai_can_search_features_by_keyword() {
    // Scenario: AI asks "What features relate to streaming?"
    let platform = PlatformRegistry::discover();

    let results = platform.search_features("streaming");
    assert!(!results.is_empty());

    // AI finds the streaming feature
    assert!(results.iter().any(|f| f.name == "streaming"));
}

#[test]
fn test_ai_can_get_feature_backends() {
    // Scenario: AI asks "What backends does streaming support?"
    let platform = PlatformRegistry::discover();

    let feature = platform.get_feature("streaming").unwrap();
    let backends = feature.backends().unwrap();

    // AI sees: ["WebSocket", "SSE", "Callback"]
    assert!(backends.contains(&"WebSocket".to_string()));
}

#[test]
fn test_ai_can_find_default_enabled_features() {
    // Scenario: AI asks "What features are enabled by default?"
    let platform = PlatformRegistry::discover();

    let defaults = platform.default_features();
    assert!(!defaults.is_empty());

    // Should include core features
    assert!(defaults.iter().any(|f| f.name == "graph_orchestration"));
}

#[test]
fn test_comprehensive_feature_catalog() {
    let registry = PlatformRegistry::discover();

    // Should have at least 12 features (8 original + 4 new catalog entries)
    assert!(
        registry.features.len() >= 12,
        "Should have at least 12 features, got {}",
        registry.features.len()
    );

    // Verify new feature catalog entries
    assert!(registry.has_feature("llm_providers"));
    assert!(registry.has_feature("vector_stores"));
    assert!(registry.has_feature("tools"));
    assert!(registry.has_feature("embeddings"));
}

// -------------------------------------------------------------------------
// Documentation Querying Tests
// -------------------------------------------------------------------------

#[test]
fn test_doc_result_creation() {
    let result = DocResult::new("Test Topic", "Test content", 0.8, "test_module");

    assert_eq!(result.title, "Test Topic");
    assert_eq!(result.content, "Test content");
    assert!((result.relevance - 0.8).abs() < f64::EPSILON);
    assert_eq!(result.source, "test_module");
    assert!(result.example.is_none());
}

#[test]
fn test_doc_result_with_example() {
    let result = DocResult::new("Test", "Content", 0.5, "module").with_example("let x = 1;");

    assert!(result.example.is_some());
    assert_eq!(result.example.unwrap(), "let x = 1;");
}

#[test]
fn test_api_docs_creation() {
    let docs = ApiDocs::new(
        "StateGraph::new",
        "Create a new state graph",
        "fn new() -> StateGraph<S>",
    );

    assert_eq!(docs.name, "StateGraph::new");
    assert_eq!(docs.description, "Create a new state graph");
    assert!(docs.parameters.is_empty());
    assert!(docs.returns.is_none());
}

#[test]
fn test_api_docs_builder() {
    let docs = ApiDocs::new("test", "desc", "fn test()")
        .add_param("x", "The x parameter")
        .add_param("y", "The y parameter")
        .returns("A result value")
        .add_example("test(1, 2)")
        .add_related("other_fn");

    assert_eq!(docs.parameters.len(), 2);
    assert_eq!(docs.parameters[0].name, "x");
    assert!(docs.returns.is_some());
    assert_eq!(docs.examples.len(), 1);
    assert_eq!(docs.related.len(), 1);
}

#[test]
fn test_documentation_query_new() {
    let docs = DocumentationQuery::new();

    // Should have topics from the registry
    let topics = docs.list_topics();
    assert!(!topics.is_empty());
}

#[test]
fn test_documentation_query_with_registry() {
    let registry = PlatformRegistry::discover();
    let docs = DocumentationQuery::with_registry(registry);

    let topics = docs.list_topics();
    assert!(!topics.is_empty());
}

#[test]
fn test_documentation_search() {
    let docs = DocumentationQuery::new();

    // Search for graph-related documentation
    let results = docs.search("graph");
    assert!(!results.is_empty());

    // Results should be sorted by relevance (descending)
    for i in 1..results.len() {
        assert!(results[i - 1].relevance >= results[i].relevance);
    }
}

#[test]
fn test_documentation_search_case_insensitive() {
    let docs = DocumentationQuery::new();

    let results_lower = docs.search("graph");
    let results_upper = docs.search("GRAPH");
    let results_mixed = docs.search("Graph");

    // Should all find results
    assert!(!results_lower.is_empty());
    assert!(!results_upper.is_empty());
    assert!(!results_mixed.is_empty());
}

#[test]
fn test_documentation_search_empty_query() {
    let docs = DocumentationQuery::new();

    let results = docs.search("");
    assert!(results.is_empty());

    let results = docs.search("   ");
    assert!(results.is_empty());
}

#[test]
fn test_documentation_search_multiple_terms() {
    let docs = DocumentationQuery::new();

    // Search with multiple terms
    let results = docs.search("add node");
    assert!(!results.is_empty());

    // Should find StateGraph::add_node
    assert!(results.iter().any(|r| r.title.contains("add_node")));
}

#[test]
fn test_documentation_get_example_api() {
    let docs = DocumentationQuery::new();

    // Should find example for StateGraph::new
    let example = docs.get_example("StateGraph::new");
    assert!(example.is_some());
    assert!(example.unwrap().contains("StateGraph"));
}

#[test]
fn test_documentation_get_example_concept() {
    let docs = DocumentationQuery::new();

    // Should find example for checkpointing concept
    let example = docs.get_example("checkpoint");
    assert!(example.is_some());
}

#[test]
fn test_documentation_get_example_not_found() {
    let docs = DocumentationQuery::new();

    let example = docs.get_example("nonexistent_api_xyz");
    assert!(example.is_none());
}

#[test]
fn test_documentation_get_api_docs() {
    let docs = DocumentationQuery::new();

    // Get docs for StateGraph::add_node
    let api_docs = docs.get_api_docs("add_node");
    assert!(api_docs.is_some());

    let api = api_docs.unwrap();
    assert!(api.name.contains("add_node"));
    assert!(!api.signature.is_empty());
}

#[test]
fn test_documentation_get_api_docs_with_related() {
    let docs = DocumentationQuery::new();

    // Get docs for a StateGraph method - should have related APIs
    let api_docs = docs.get_api_docs("StateGraph::add_node");
    assert!(api_docs.is_some());

    let api = api_docs.unwrap();
    // Should have related StateGraph methods
    assert!(!api.related.is_empty());
}

#[test]
fn test_documentation_get_api_docs_not_found() {
    let docs = DocumentationQuery::new();

    let api_docs = docs.get_api_docs("nonexistent_fn");
    assert!(api_docs.is_none());
}

#[test]
fn test_documentation_list_topics() {
    let docs = DocumentationQuery::new();

    let topics = docs.list_topics();
    assert!(!topics.is_empty());

    // Should include module names and API names
    assert!(topics
        .iter()
        .any(|t| t.contains("StateGraph") || t.contains("core")));
}

#[test]
fn test_documentation_all_examples() {
    let docs = DocumentationQuery::new();

    let examples = docs.all_examples();
    assert!(!examples.is_empty());

    // Each example should have both topic and example content
    for (topic, example) in examples {
        assert!(!topic.is_empty());
        assert!(!example.is_empty());
    }
}

#[test]
fn test_documentation_topics_in_module() {
    let docs = DocumentationQuery::new();

    // Get topics in core module
    let core_topics = docs.topics_in_module("core");
    assert!(!core_topics.is_empty());

    // Non-existent module should return empty
    let empty = docs.topics_in_module("nonexistent_module");
    assert!(empty.is_empty());
}

#[test]
fn test_documentation_search_finds_concept_docs() {
    let docs = DocumentationQuery::new();

    // Should find "Getting Started" concept
    let results = docs.search("getting started");
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.title == "Getting Started"));

    // Should find "Conditional Routing" concept
    let results = docs.search("conditional routing");
    assert!(!results.is_empty());
}

#[test]
fn test_documentation_search_finds_feature_docs() {
    let docs = DocumentationQuery::new();

    // Should find checkpointing feature
    let results = docs.search("persistence");
    assert!(!results.is_empty());
}

#[test]
fn test_documentation_search_relevance() {
    let docs = DocumentationQuery::new();

    // Search for something specific
    let results = docs.search("StateGraph");

    // Results should have relevance between 0 and 1
    for result in &results {
        assert!(result.relevance >= 0.0);
        assert!(result.relevance <= 1.0);
    }
}

// AI usage scenario tests

#[test]
fn test_ai_can_search_documentation() {
    // Scenario: AI asks "How do I add a node to my graph?"
    let docs = DocumentationQuery::new();

    let results = docs.search("add node");
    assert!(!results.is_empty());

    // AI finds the StateGraph::add_node API
    assert!(results.iter().any(|r| r.title.contains("add_node")));
}

#[test]
fn test_ai_can_get_code_example() {
    // Scenario: AI asks "Show me an example of creating a StateGraph"
    let docs = DocumentationQuery::new();

    let example = docs.get_example("StateGraph");
    assert!(example.is_some());

    // AI sees actual code
    let code = example.unwrap();
    assert!(code.contains("StateGraph") || code.contains("graph"));
}

#[test]
fn test_ai_can_get_api_documentation() {
    // Scenario: AI asks "What is the signature of compile?"
    let docs = DocumentationQuery::new();

    let api = docs.get_api_docs("compile");
    assert!(api.is_some());

    let api = api.unwrap();
    // AI sees signature
    assert!(!api.signature.is_empty());
}

#[test]
fn test_ai_can_learn_concepts() {
    // Scenario: AI asks "How do I do checkpointing?"
    let docs = DocumentationQuery::new();

    let results = docs.search("checkpoint save state");
    assert!(!results.is_empty());

    // Should find results with examples
    let with_examples: Vec<_> = results.iter().filter(|r| r.example.is_some()).collect();
    assert!(!with_examples.is_empty());
}

#[test]
fn test_ai_can_discover_streaming() {
    // Scenario: AI asks "How do I stream execution events?"
    let docs = DocumentationQuery::new();

    let example = docs.get_example("stream");
    assert!(example.is_some());

    // AI sees streaming code example
    let code = example.unwrap();
    assert!(code.contains("stream") || code.contains("Stream"));
}

#[test]
fn test_ai_can_learn_loops() {
    // Scenario: AI asks "How do I create a loop in my graph?"
    let docs = DocumentationQuery::new();

    let results = docs.search("loop cycle iterate");
    assert!(!results.is_empty());

    // Should find cycles documentation
    assert!(results
        .iter()
        .any(|r| r.title.contains("Cycle") || r.title.contains("Loop")));
}

#[test]
fn test_ai_can_browse_module() {
    // Scenario: AI asks "What APIs are in the core module?"
    let docs = DocumentationQuery::new();

    let topics = docs.topics_in_module("core");
    assert!(!topics.is_empty());

    // AI sees all core module topics
    assert!(topics.iter().any(|t| t.contains("StateGraph")));
}

#[test]
fn test_doc_result_serialization() {
    let result = DocResult::new("Test", "Content", 0.5, "module").with_example("code");

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("Test"));
    assert!(json.contains("code"));

    let parsed: DocResult = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.title, "Test");
}

#[test]
fn test_api_docs_serialization() {
    let docs = ApiDocs::new("test", "desc", "fn test()")
        .add_example("example")
        .add_related("other");

    let json = serde_json::to_string(&docs).unwrap();
    assert!(json.contains("test"));

    let parsed: ApiDocs = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "test");
}

// -------------------------------------------------------------------------
// App Architecture Analysis Tests
// -------------------------------------------------------------------------

#[test]
fn test_app_architecture_builder() {
    let arch = AppArchitecture::builder()
        .graph_structure(ArchitectureGraphInfo::new("start"))
        .metadata(ArchitectureMetadata::new())
        .build();

    assert_eq!(arch.graph_structure.entry_point, "start");
    assert!(!arch.metadata.dashflow_version.is_empty());
}

#[test]
fn test_architecture_graph_info_creation() {
    let info = ArchitectureGraphInfo::new("entry")
        .with_name("Test Graph")
        .with_node_count(5)
        .with_edge_count(4)
        .with_node_names(vec!["a".to_string(), "b".to_string()])
        .with_cycles(true)
        .with_conditional_edges(true)
        .with_parallel_edges(false);

    assert_eq!(info.name, Some("Test Graph".to_string()));
    assert_eq!(info.entry_point, "entry");
    assert_eq!(info.node_count, 5);
    assert_eq!(info.edge_count, 4);
    assert_eq!(info.node_names.len(), 2);
    assert!(info.has_cycles);
    assert!(info.has_conditional_edges);
    assert!(!info.has_parallel_edges);
}

#[test]
fn test_feature_usage_creation() {
    let feature = FeatureUsage::new("StateGraph", "core", "Graph orchestration")
        .with_api("StateGraph::new")
        .with_apis(vec!["StateGraph::compile", "CompiledGraph::invoke"])
        .core();

    assert_eq!(feature.name, "StateGraph");
    assert_eq!(feature.category, "core");
    assert!(feature.is_core);
    assert_eq!(feature.apis_used.len(), 3);
}

#[test]
fn test_code_module_creation() {
    let module = CodeModule::new("reasoning_node")
        .with_file("src/nodes/reasoning.rs")
        .with_lines(245)
        .with_api("Message")
        .with_apis(vec!["ChatModel", "Tool"])
        .with_description("Handles AI reasoning");

    assert_eq!(module.name, "reasoning_node");
    assert_eq!(module.file, Some("src/nodes/reasoning.rs".to_string()));
    assert_eq!(module.lines, 245);
    assert_eq!(module.dashflow_apis_used.len(), 3);
    assert!(module.description.is_some());
}

#[test]
fn test_dependency_creation() {
    let dep = Dependency::new("dashflow-openai", "OpenAI LLM provider")
        .with_version("1.0.0")
        .dashflow()
        .with_api("ChatOpenAI");

    assert_eq!(dep.name, "dashflow-openai");
    assert_eq!(dep.version, Some("1.0.0".to_string()));
    assert!(dep.is_dashflow);
    assert_eq!(dep.apis_used.len(), 1);
}

#[test]
fn test_architecture_metadata() {
    let metadata = ArchitectureMetadata::new()
        .with_note("Test note 1")
        .with_note("Test note 2");

    assert!(!metadata.dashflow_version.is_empty());
    assert_eq!(metadata.notes.len(), 2);
}

#[test]
fn test_app_architecture_summary() {
    let mut builder = AppArchitecture::builder().graph_structure(
        ArchitectureGraphInfo::new("start")
            .with_node_count(3)
            .with_edge_count(2),
    );

    builder.add_feature(FeatureUsage::new("StateGraph", "core", "Core feature"));
    builder.add_code_module(CodeModule::new("test_module"));
    builder.add_dependency(Dependency::new("test", "Testing"));

    let arch = builder.build();
    let summary = arch.summary();

    assert!(summary.contains("3 nodes"));
    assert!(summary.contains("2 edges"));
    assert!(summary.contains("1 DashFlow features"));
    assert!(summary.contains("1 custom modules"));
    assert!(summary.contains("1 dependencies"));
}

#[test]
fn test_app_architecture_features_by_category() {
    let mut builder = AppArchitecture::builder();
    builder.add_feature(FeatureUsage::new("StateGraph", "core", "Core feature"));
    builder.add_feature(FeatureUsage::new(
        "Checkpointing",
        "checkpoint",
        "Persistence",
    ));
    builder.add_feature(FeatureUsage::new("Streaming", "streaming", "Real-time"));

    let arch = builder.build();

    let core_features = arch.features_by_category("core");
    assert_eq!(core_features.len(), 1);
    assert_eq!(core_features[0].name, "StateGraph");

    let checkpoint_features = arch.features_by_category("checkpoint");
    assert_eq!(checkpoint_features.len(), 1);
}

#[test]
fn test_app_architecture_uses_feature() {
    let mut builder = AppArchitecture::builder();
    builder.add_feature(FeatureUsage::new("StateGraph", "core", "Core feature"));

    let arch = builder.build();

    assert!(arch.uses_feature("StateGraph"));
    assert!(arch.uses_feature("stategraph")); // case insensitive
    assert!(!arch.uses_feature("NonexistentFeature"));
}

#[test]
fn test_app_architecture_total_custom_lines() {
    let mut builder = AppArchitecture::builder();
    builder.add_code_module(CodeModule::new("module1").with_lines(100));
    builder.add_code_module(CodeModule::new("module2").with_lines(200));
    builder.add_code_module(CodeModule::new("module3").with_lines(150));

    let arch = builder.build();

    assert_eq!(arch.total_custom_lines(), 450);
}

#[test]
fn test_app_architecture_dependency_filtering() {
    let mut builder = AppArchitecture::builder();
    builder.add_dependency(Dependency::new("dashflow", "Core").dashflow());
    builder.add_dependency(Dependency::new("dashflow-openai", "OpenAI").dashflow());
    builder.add_dependency(Dependency::new("tokio", "Async runtime"));
    builder.add_dependency(Dependency::new("serde", "Serialization"));

    let arch = builder.build();

    let dashflow_deps = arch.dashflow_dependencies();
    assert_eq!(dashflow_deps.len(), 2);

    let external_deps = arch.external_dependencies();
    assert_eq!(external_deps.len(), 2);
}

#[test]
fn test_app_architecture_json_serialization() {
    let mut builder = AppArchitecture::builder()
        .graph_structure(ArchitectureGraphInfo::new("start").with_node_count(2));
    builder.add_feature(FeatureUsage::new("Test", "core", "Description"));

    let arch = builder.build();
    let json = arch.to_json().unwrap();

    assert!(json.contains("start"));
    assert!(json.contains("Test"));
    assert!(json.contains("dashflow_features_used"));

    // Should be parseable
    let parsed: AppArchitecture = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.graph_structure.entry_point, "start");
}

#[test]
fn test_feature_usage_json_serialization() {
    let feature = FeatureUsage::new("StateGraph", "core", "Core orchestration")
        .with_api("StateGraph::new")
        .core();

    let json = serde_json::to_string(&feature).unwrap();
    assert!(json.contains("StateGraph"));
    assert!(json.contains("is_core"));

    let parsed: FeatureUsage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "StateGraph");
    assert!(parsed.is_core);
}

#[test]
fn test_code_module_json_serialization() {
    let module = CodeModule::new("test_module")
        .with_file("src/test.rs")
        .with_lines(100);

    let json = serde_json::to_string(&module).unwrap();
    assert!(json.contains("test_module"));

    let parsed: CodeModule = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "test_module");
}

#[test]
fn test_dependency_json_serialization() {
    let dep = Dependency::new("dashflow", "Core framework")
        .with_version("1.0.0")
        .dashflow();

    let json = serde_json::to_string(&dep).unwrap();
    assert!(json.contains("dashflow"));
    assert!(json.contains("1.0.0"));

    let parsed: Dependency = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "dashflow");
    assert!(parsed.is_dashflow);
}

#[test]
fn test_architecture_graph_info_default() {
    let info = ArchitectureGraphInfo::default();

    assert!(info.name.is_none());
    assert!(info.entry_point.is_empty());
    assert_eq!(info.node_count, 0);
    assert_eq!(info.edge_count, 0);
    assert!(!info.has_cycles);
}

// AI usage scenario tests

#[test]
fn test_ai_can_understand_graph_structure() {
    // Scenario: AI asks "What is my graph structure?"
    let arch = AppArchitecture::builder()
        .graph_structure(
            ArchitectureGraphInfo::new("user_input")
                .with_name("Coding Agent")
                .with_node_count(5)
                .with_edge_count(6)
                .with_node_names(vec![
                    "user_input".to_string(),
                    "reasoning".to_string(),
                    "tool_selection".to_string(),
                    "tool_execution".to_string(),
                    "output".to_string(),
                ])
                .with_cycles(true)
                .with_conditional_edges(true),
        )
        .build();

    // AI sees graph structure
    assert_eq!(arch.graph_structure.name, Some("Coding Agent".to_string()));
    assert_eq!(arch.graph_structure.node_count, 5);
    assert!(arch.graph_structure.has_cycles);
    assert!(arch.graph_structure.has_conditional_edges);
}

#[test]
fn test_ai_can_list_features_used() {
    // Scenario: AI asks "What DashFlow features am I using?"
    let mut builder = AppArchitecture::builder();
    builder.add_feature(
        FeatureUsage::new("StateGraph", "core", "Graph-based workflow")
            .with_apis(vec!["StateGraph::new", "compile", "invoke"])
            .core(),
    );
    builder.add_feature(
        FeatureUsage::new("dashflow-openai", "llm", "OpenAI LLM provider").with_api("ChatOpenAI"),
    );
    builder.add_feature(
        FeatureUsage::new("Checkpointing", "checkpoint", "State persistence")
            .with_api("MemoryCheckpointer"),
    );

    let arch = builder.build();

    // AI sees what features are used
    assert_eq!(arch.dashflow_features_used.len(), 3);
    assert!(arch.uses_feature("StateGraph"));
    assert!(arch.uses_feature("dashflow-openai"));
}

#[test]
fn test_ai_can_understand_custom_code() {
    // Scenario: AI asks "Where is my custom code?"
    let mut builder = AppArchitecture::builder();
    builder.add_code_module(
        CodeModule::new("reasoning_node")
            .with_file("src/nodes/reasoning.rs")
            .with_lines(245)
            .with_apis(vec!["Message", "ChatModel"])
            .with_description("AI reasoning using GPT-4"),
    );
    builder.add_code_module(
        CodeModule::new("tool_execution")
            .with_file("src/nodes/tools.rs")
            .with_lines(180)
            .with_apis(vec!["Tool", "SafeShellTool"]),
    );

    let arch = builder.build();

    // AI sees custom code modules
    assert_eq!(arch.custom_code.len(), 2);
    assert_eq!(arch.total_custom_lines(), 425);

    let reasoning = &arch.custom_code[0];
    assert_eq!(reasoning.name, "reasoning_node");
    assert_eq!(reasoning.lines, 245);
}

#[test]
fn test_ai_can_understand_dependencies() {
    // Scenario: AI asks "What dependencies do I have?"
    let mut builder = AppArchitecture::builder();
    builder.add_dependency(
        Dependency::new("dashflow", "Core orchestration framework")
            .with_version("1.11.2")
            .dashflow(),
    );
    builder.add_dependency(
        Dependency::new("dashflow-openai", "OpenAI integration")
            .with_version("1.11.2")
            .dashflow()
            .with_api("ChatOpenAI"),
    );
    builder.add_dependency(Dependency::new("tokio", "Async runtime").with_version("1.35.0"));

    let arch = builder.build();

    // AI sees dependencies
    assert_eq!(arch.dependencies.len(), 3);
    assert_eq!(arch.dashflow_dependencies().len(), 2);
    assert_eq!(arch.external_dependencies().len(), 1);
}

#[test]
fn test_ai_can_get_architecture_summary() {
    // Scenario: AI asks "How am I built?" (summary)
    let mut builder = AppArchitecture::builder().graph_structure(
        ArchitectureGraphInfo::new("start")
            .with_node_count(5)
            .with_edge_count(6),
    );

    builder.add_feature(FeatureUsage::new("StateGraph", "core", "Core"));
    builder.add_feature(FeatureUsage::new("OpenAI", "llm", "LLM"));
    builder.add_code_module(CodeModule::new("reasoning").with_lines(200));
    builder.add_dependency(Dependency::new("dashflow", "Core").dashflow());

    let arch = builder.build();
    let summary = arch.summary();

    // AI gets a quick summary
    assert!(summary.contains("5 nodes"));
    assert!(summary.contains("6 edges"));
    assert!(summary.contains("2 DashFlow features"));
}

#[test]
fn test_complete_architecture_scenario() {
    // Complete scenario: Build a full architecture like a real AI agent
    let mut builder = AppArchitecture::builder()
        .graph_structure(
            ArchitectureGraphInfo::new("user_input")
                .with_name("Claude Code Agent")
                .with_node_count(5)
                .with_edge_count(7)
                .with_node_names(vec![
                    "user_input".to_string(),
                    "reasoning".to_string(),
                    "tool_selection".to_string(),
                    "tool_execution".to_string(),
                    "output".to_string(),
                ])
                .with_cycles(true)
                .with_conditional_edges(true),
        )
        .metadata(ArchitectureMetadata::new().with_note("Coding assistant agent"));

    // Core features
    builder.add_feature(
        FeatureUsage::new("StateGraph", "core", "Graph orchestration")
            .with_apis(vec!["StateGraph::new", "compile", "invoke"])
            .core(),
    );
    builder.add_feature(
        FeatureUsage::new("Conditional Routing", "core", "Dynamic routing")
            .with_api("add_conditional_edges"),
    );
    builder.add_feature(
        FeatureUsage::new("Cycles", "core", "Iterative execution").with_api("add_edge"),
    );

    // LLM integration
    builder.add_feature(
        FeatureUsage::new("dashflow-openai", "llm", "GPT-4 provider").with_api("ChatOpenAI"),
    );

    // Tools
    builder.add_feature(
        FeatureUsage::new("dashflow-shell-tool", "tools", "Shell execution")
            .with_api("SafeShellTool"),
    );

    // Custom code
    builder.add_code_module(
        CodeModule::new("reasoning_node")
            .with_file("src/nodes/reasoning.rs")
            .with_lines(245)
            .with_description("AI reasoning using GPT-4"),
    );

    // Dependencies
    builder.add_dependency(
        Dependency::new("dashflow", "Core framework")
            .with_version("1.11.2")
            .dashflow(),
    );

    let arch = builder.build();

    // Verify complete architecture
    assert_eq!(
        arch.graph_structure.name,
        Some("Claude Code Agent".to_string())
    );
    assert_eq!(arch.dashflow_features_used.len(), 5);
    assert_eq!(arch.custom_code.len(), 1);
    assert_eq!(arch.dependencies.len(), 1);
    assert!(arch.uses_feature("StateGraph"));
    assert!(arch.uses_feature("dashflow-openai"));

    // Should serialize to JSON
    let json = arch.to_json().unwrap();
    assert!(json.contains("Claude Code Agent"));
}

// -------------------------------------------------------------------------
// Dependency Analysis Tests
// -------------------------------------------------------------------------

#[test]
fn test_dependency_analysis_builder() {
    let analysis = DependencyAnalysisBuilder::new()
        .dashflow_version("1.11.2")
        .build();

    assert_eq!(analysis.dashflow_version, "1.11.2");
    assert!(analysis.dashflow_crates.is_empty());
    assert!(analysis.external_crates.is_empty());
}

#[test]
fn test_dependency_analysis_default_version() {
    let analysis = DependencyAnalysisBuilder::new().build();

    // Should use CARGO_PKG_VERSION
    assert!(!analysis.dashflow_version.is_empty());
}

#[test]
fn test_crate_dependency_creation() {
    let crate_dep = CrateDependency::new("dashflow-openai", "OpenAI LLM provider")
        .with_version("1.11.2")
        .with_api("ChatOpenAI")
        .with_apis(vec!["OpenAIEmbeddings", "complete"])
        .with_category(DependencyCategory::LlmProvider)
        .with_features(vec!["streaming", "function_calling"]);

    assert_eq!(crate_dep.name, "dashflow-openai");
    assert_eq!(crate_dep.purpose, "OpenAI LLM provider");
    assert_eq!(crate_dep.version, Some("1.11.2".to_string()));
    assert_eq!(crate_dep.apis_used.len(), 3);
    assert_eq!(crate_dep.category, DependencyCategory::LlmProvider);
    assert_eq!(crate_dep.features.len(), 2);
    assert!(crate_dep.is_direct);
    assert!(!crate_dep.optional);
}

#[test]
fn test_crate_dependency_transitive_optional() {
    let crate_dep = CrateDependency::new("test", "Test crate")
        .transitive()
        .optional();

    assert!(!crate_dep.is_direct);
    assert!(crate_dep.optional);
}

#[test]
fn test_crate_dependency_uses_api() {
    let crate_dep =
        CrateDependency::new("test", "Test").with_apis(vec!["ChatOpenAI", "OpenAIEmbeddings"]);

    assert!(crate_dep.uses_api("ChatOpenAI"));
    assert!(crate_dep.uses_api("chatopenai")); // case insensitive
    assert!(crate_dep.uses_api("Embeddings")); // partial match
    assert!(!crate_dep.uses_api("Anthropic"));
}

#[test]
fn test_dependency_category_display() {
    assert_eq!(format!("{}", DependencyCategory::Core), "Core");
    assert_eq!(
        format!("{}", DependencyCategory::LlmProvider),
        "LLM Provider"
    );
    assert_eq!(
        format!("{}", DependencyCategory::VectorStore),
        "Vector Store"
    );
    assert_eq!(format!("{}", DependencyCategory::Runtime), "Runtime");
    assert_eq!(
        format!("{}", DependencyCategory::Serialization),
        "Serialization"
    );
    assert_eq!(format!("{}", DependencyCategory::Networking), "Networking");
    assert_eq!(
        format!("{}", DependencyCategory::Observability),
        "Observability"
    );
    assert_eq!(format!("{}", DependencyCategory::Testing), "Testing");
}

#[test]
fn test_dependency_category_default() {
    let category: DependencyCategory = Default::default();
    assert_eq!(category, DependencyCategory::Other);
}

#[test]
fn test_dependency_metadata() {
    let metadata = DependencyMetadata::new()
        .with_source("Cargo.toml")
        .with_cargo_path("/path/to/Cargo.toml")
        .with_note("Test note");

    assert_eq!(metadata.source, Some("Cargo.toml".to_string()));
    assert_eq!(
        metadata.cargo_toml_path,
        Some("/path/to/Cargo.toml".to_string())
    );
    assert_eq!(metadata.notes.len(), 1);
}

#[test]
fn test_dependency_analysis_add_crates() {
    let mut builder = DependencyAnalysisBuilder::new().dashflow_version("1.11.2");

    builder.add_dashflow_crate(CrateDependency::new("dashflow", "Core"));
    builder.add_dashflow_crate(CrateDependency::new("dashflow-openai", "OpenAI"));
    builder.add_external_crate(CrateDependency::new("tokio", "Runtime"));
    builder.add_external_crate(CrateDependency::new("serde", "Serialization"));

    let analysis = builder.build();

    assert_eq!(analysis.dashflow_crates.len(), 2);
    assert_eq!(analysis.external_crates.len(), 2);
    assert_eq!(analysis.total_crates(), 4);
}

#[test]
fn test_dependency_analysis_find_crate() {
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_dashflow_crate(CrateDependency::new("dashflow-openai", "OpenAI"));
    builder.add_external_crate(CrateDependency::new("tokio", "Runtime"));

    let analysis = builder.build();

    assert!(analysis.find_crate("dashflow-openai").is_some());
    assert!(analysis.find_crate("DASHFLOW-OPENAI").is_some()); // case insensitive
    assert!(analysis.find_crate("tokio").is_some());
    assert!(analysis.find_crate("nonexistent").is_none());
}

#[test]
fn test_dependency_analysis_uses_crate() {
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_dashflow_crate(CrateDependency::new("dashflow", "Core"));

    let analysis = builder.build();

    assert!(analysis.uses_crate("dashflow"));
    assert!(!analysis.uses_crate("nonexistent"));
}

#[test]
fn test_dependency_analysis_crates_by_category() {
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_dashflow_crate(
        CrateDependency::new("dashflow-openai", "OpenAI")
            .with_category(DependencyCategory::LlmProvider),
    );
    builder.add_dashflow_crate(
        CrateDependency::new("dashflow-anthropic", "Anthropic")
            .with_category(DependencyCategory::LlmProvider),
    );
    builder.add_external_crate(
        CrateDependency::new("tokio", "Runtime").with_category(DependencyCategory::Runtime),
    );

    let analysis = builder.build();

    let llm_crates = analysis.crates_by_category(DependencyCategory::LlmProvider);
    assert_eq!(llm_crates.len(), 2);

    let runtime_crates = analysis.crates_by_category(DependencyCategory::Runtime);
    assert_eq!(runtime_crates.len(), 1);
}

#[test]
fn test_dependency_analysis_llm_provider_crates() {
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_dashflow_crate(
        CrateDependency::new("dashflow-openai", "OpenAI")
            .with_category(DependencyCategory::LlmProvider),
    );

    let analysis = builder.build();

    let llm_crates = analysis.llm_provider_crates();
    assert_eq!(llm_crates.len(), 1);
    assert_eq!(llm_crates[0].name, "dashflow-openai");
}

#[test]
fn test_dependency_analysis_vector_store_crates() {
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_dashflow_crate(
        CrateDependency::new("dashflow-chroma", "Chroma")
            .with_category(DependencyCategory::VectorStore),
    );

    let analysis = builder.build();

    let vs_crates = analysis.vector_store_crates();
    assert_eq!(vs_crates.len(), 1);
}

#[test]
fn test_dependency_analysis_tool_crates() {
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_dashflow_crate(
        CrateDependency::new("dashflow-shell-tool", "Shell")
            .with_category(DependencyCategory::Tool),
    );

    let analysis = builder.build();

    let tool_crates = analysis.tool_crates();
    assert_eq!(tool_crates.len(), 1);
}

#[test]
fn test_dependency_analysis_checkpoint_crates() {
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_dashflow_crate(
        CrateDependency::new("dashflow-redis-checkpointer", "Redis checkpoint")
            .with_category(DependencyCategory::Checkpointer),
    );

    let analysis = builder.build();

    let cp_crates = analysis.checkpoint_crates();
    assert_eq!(cp_crates.len(), 1);
}

#[test]
fn test_dependency_analysis_runtime_crates() {
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_external_crate(
        CrateDependency::new("tokio", "Runtime").with_category(DependencyCategory::Runtime),
    );

    let analysis = builder.build();

    let runtime_crates = analysis.runtime_crates();
    assert_eq!(runtime_crates.len(), 1);
}

#[test]
fn test_dependency_analysis_summary() {
    let mut builder = DependencyAnalysisBuilder::new().dashflow_version("1.11.2");
    builder.add_dashflow_crate(CrateDependency::new("dashflow", "Core"));
    builder.add_dashflow_crate(CrateDependency::new("dashflow-openai", "OpenAI"));
    builder.add_external_crate(CrateDependency::new("tokio", "Runtime"));

    let analysis = builder.build();
    let summary = analysis.summary();

    assert!(summary.contains("3 total"));
    assert!(summary.contains("2 DashFlow"));
    assert!(summary.contains("1 external"));
    assert!(summary.contains("1.11.2"));
}

#[test]
fn test_dependency_analysis_crates_using_api() {
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_dashflow_crate(
        CrateDependency::new("dashflow-openai", "OpenAI")
            .with_apis(vec!["ChatOpenAI", "OpenAIEmbeddings"]),
    );
    builder.add_dashflow_crate(
        CrateDependency::new("dashflow-anthropic", "Anthropic").with_api("ChatAnthropic"),
    );

    let analysis = builder.build();

    let using_chat = analysis.crates_using_api("Chat");
    assert_eq!(using_chat.len(), 2);

    let using_openai = analysis.crates_using_api("OpenAI");
    assert_eq!(using_openai.len(), 1);
}

#[test]
fn test_dependency_analysis_all_apis_used() {
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_dashflow_crate(
        CrateDependency::new("dashflow-openai", "OpenAI")
            .with_apis(vec!["ChatOpenAI", "OpenAIEmbeddings"]),
    );
    builder.add_external_crate(CrateDependency::new("tokio", "Runtime").with_api("Runtime::new"));

    let analysis = builder.build();

    let all_apis = analysis.all_apis_used();
    assert_eq!(all_apis.len(), 3);
    assert!(all_apis.contains(&"ChatOpenAI"));
    assert!(all_apis.contains(&"Runtime::new"));
}

#[test]
fn test_dependency_analysis_json_serialization() {
    let mut builder = DependencyAnalysisBuilder::new().dashflow_version("1.11.2");
    builder.add_dashflow_crate(
        CrateDependency::new("dashflow", "Core").with_category(DependencyCategory::Core),
    );

    let analysis = builder.build();
    let json = analysis.to_json().unwrap();

    assert!(json.contains("1.11.2"));
    assert!(json.contains("dashflow"));
    assert!(json.contains("dashflow_crates"));

    // Should be parseable
    let parsed: DependencyAnalysis = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.dashflow_version, "1.11.2");
}

#[test]
fn test_crate_dependency_json_serialization() {
    let crate_dep = CrateDependency::new("dashflow-openai", "OpenAI provider")
        .with_version("1.11.2")
        .with_category(DependencyCategory::LlmProvider)
        .with_features(vec!["streaming"]);

    let json = serde_json::to_string(&crate_dep).unwrap();
    assert!(json.contains("dashflow-openai"));
    assert!(json.contains("llm_provider")); // snake_case from serde

    let parsed: CrateDependency = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "dashflow-openai");
    assert_eq!(parsed.category, DependencyCategory::LlmProvider);
}

#[test]
fn test_infer_category_llm_providers() {
    assert_eq!(
        AppArchitecture::infer_category("dashflow-openai"),
        DependencyCategory::LlmProvider
    );
    assert_eq!(
        AppArchitecture::infer_category("dashflow-anthropic"),
        DependencyCategory::LlmProvider
    );
    assert_eq!(
        AppArchitecture::infer_category("dashflow-bedrock"),
        DependencyCategory::LlmProvider
    );
    assert_eq!(
        AppArchitecture::infer_category("dashflow-gemini"),
        DependencyCategory::LlmProvider
    );
    assert_eq!(
        AppArchitecture::infer_category("dashflow-ollama"),
        DependencyCategory::LlmProvider
    );
}

#[test]
fn test_infer_category_vector_stores() {
    assert_eq!(
        AppArchitecture::infer_category("dashflow-chroma"),
        DependencyCategory::VectorStore
    );
    assert_eq!(
        AppArchitecture::infer_category("dashflow-pinecone"),
        DependencyCategory::VectorStore
    );
    assert_eq!(
        AppArchitecture::infer_category("dashflow-qdrant"),
        DependencyCategory::VectorStore
    );
    assert_eq!(
        AppArchitecture::infer_category("dashflow-pgvector"),
        DependencyCategory::VectorStore
    );
}

#[test]
fn test_infer_category_tools() {
    assert_eq!(
        AppArchitecture::infer_category("dashflow-shell-tool"),
        DependencyCategory::Tool
    );
    assert_eq!(
        AppArchitecture::infer_category("dashflow-file-tool"),
        DependencyCategory::Tool
    );
    assert_eq!(
        AppArchitecture::infer_category("dashflow-calculator"),
        DependencyCategory::Tool
    );
}

#[test]
fn test_infer_category_checkpointers() {
    assert_eq!(
        AppArchitecture::infer_category("dashflow-redis-checkpointer"),
        DependencyCategory::Checkpointer
    );
    assert_eq!(
        AppArchitecture::infer_category("dashflow-postgres-checkpointer"),
        DependencyCategory::Checkpointer
    );
}

#[test]
fn test_infer_category_external() {
    assert_eq!(
        AppArchitecture::infer_category("tokio"),
        DependencyCategory::Runtime
    );
    assert_eq!(
        AppArchitecture::infer_category("serde"),
        DependencyCategory::Serialization
    );
    assert_eq!(
        AppArchitecture::infer_category("reqwest"),
        DependencyCategory::Networking
    );
    assert_eq!(
        AppArchitecture::infer_category("tracing"),
        DependencyCategory::Observability
    );
}

#[test]
fn test_infer_category_core() {
    assert_eq!(
        AppArchitecture::infer_category("dashflow"),
        DependencyCategory::Core
    );
    assert_eq!(
        AppArchitecture::infer_category("dashflow-streaming"),
        DependencyCategory::Core
    );
}

#[test]
fn test_app_architecture_dependency_analysis() {
    let mut builder = AppArchitecture::builder().metadata(ArchitectureMetadata::new());

    builder.add_dependency(
        Dependency::new("dashflow", "Core framework")
            .with_version("1.11.2")
            .dashflow(),
    );
    builder.add_dependency(
        Dependency::new("dashflow-openai", "OpenAI provider")
            .with_version("1.11.2")
            .dashflow()
            .with_api("ChatOpenAI"),
    );
    builder.add_dependency(Dependency::new("tokio", "Async runtime").with_version("1.35.0"));

    let arch = builder.build();
    let analysis = arch.dependency_analysis();

    assert_eq!(analysis.dashflow_crates.len(), 2);
    assert_eq!(analysis.external_crates.len(), 1);
    assert!(!analysis.dashflow_version.is_empty());
}

#[test]
fn test_parse_cargo_toml_simple() {
    let toml = r#"
[dependencies]
dashflow = "1.11.2"
dashflow-openai = "1.11.2"
tokio = "1.35.0"
serde = "1.0"
"#;

    let analysis = parse_cargo_toml(toml);

    assert_eq!(analysis.dashflow_version, "1.11.2");
    assert_eq!(analysis.dashflow_crates.len(), 2);
    assert_eq!(analysis.external_crates.len(), 2);
    assert!(analysis.uses_crate("dashflow"));
    assert!(analysis.uses_crate("tokio"));
}

#[test]
fn test_parse_cargo_toml_with_features() {
    let toml = r#"
[dependencies]
tokio = { version = "1.35", features = ["full", "macros"] }
serde = { version = "1.0", features = ["derive"] }
"#;

    let analysis = parse_cargo_toml(toml);

    let tokio = analysis.find_crate("tokio").unwrap();
    assert_eq!(tokio.version, Some("1.35".to_string()));
    assert!(tokio.features.contains(&"full".to_string()));
    assert!(tokio.features.contains(&"macros".to_string()));
}

#[test]
fn test_parse_cargo_toml_dev_dependencies() {
    let toml = r#"
[dependencies]
dashflow = "1.11.2"

[dev-dependencies]
mockall = "0.11"
"#;

    let analysis = parse_cargo_toml(toml);

    let mockall = analysis.find_crate("mockall").unwrap();
    assert!(mockall.optional);
}

#[test]
fn test_infer_purpose_dashflow_crates() {
    assert_eq!(
        infer_purpose("dashflow"),
        "Core graph orchestration framework"
    );
    assert_eq!(
        infer_purpose("dashflow-openai"),
        "OpenAI LLM provider (GPT-4, GPT-3.5)"
    );
    assert_eq!(
        infer_purpose("dashflow-anthropic"),
        "Anthropic LLM provider (Claude)"
    );
    assert_eq!(infer_purpose("dashflow-chroma"), "Chroma vector store");
    assert_eq!(
        infer_purpose("dashflow-redis-checkpointer"),
        "Redis checkpoint backend"
    );
}

#[test]
fn test_infer_purpose_external_crates() {
    assert_eq!(infer_purpose("tokio"), "Async runtime for Rust");
    assert_eq!(
        infer_purpose("serde"),
        "Serialization/deserialization framework"
    );
    assert_eq!(infer_purpose("reqwest"), "HTTP client");
    assert_eq!(infer_purpose("tracing"), "Application-level tracing");
}

#[test]
fn test_extract_version_simple() {
    assert_eq!(extract_version("\"1.0.0\""), Some("1.0.0".to_string()));
    assert_eq!(extract_version("\"1.35\""), Some("1.35".to_string()));
}

#[test]
fn test_extract_version_table() {
    assert_eq!(
        extract_version("{ version = \"1.0.0\" }"),
        Some("1.0.0".to_string())
    );
    assert_eq!(
        extract_version("{ version = \"1.35\", features = [\"full\"] }"),
        Some("1.35".to_string())
    );
}

#[test]
fn test_extract_features() {
    let features = extract_features("{ version = \"1.0\", features = [\"full\", \"macros\"] }");
    assert_eq!(features.len(), 2);
    assert!(features.contains(&"full".to_string()));
    assert!(features.contains(&"macros".to_string()));
}

#[test]
fn test_extract_features_empty() {
    let features = extract_features("\"1.0.0\"");
    assert!(features.is_empty());
}

// AI usage scenario tests

#[test]
fn test_ai_can_ask_dashflow_version() {
    // Scenario: AI asks "What version of DashFlow am I using?"
    let analysis = DependencyAnalysisBuilder::new()
        .dashflow_version("1.11.2")
        .build();

    assert_eq!(analysis.dashflow_version, "1.11.2");
}

#[test]
fn test_ai_can_list_dashflow_crates() {
    // Scenario: AI asks "What DashFlow crates am I using?"
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_dashflow_crate(CrateDependency::new("dashflow", "Core"));
    builder.add_dashflow_crate(CrateDependency::new("dashflow-openai", "OpenAI"));
    builder.add_dashflow_crate(CrateDependency::new("dashflow-shell-tool", "Shell"));

    let analysis = builder.build();

    // AI sees list of DashFlow crates
    assert_eq!(analysis.dashflow_crates.len(), 3);
    for crate_dep in &analysis.dashflow_crates {
        assert!(crate_dep.name.starts_with("dashflow"));
    }
}

#[test]
fn test_ai_can_ask_why_dependency() {
    // Scenario: AI asks "Why do I depend on tokio?"
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_external_crate(
        CrateDependency::new("tokio", "Async runtime for Rust")
            .with_apis(vec!["Runtime::new", "spawn"]),
    );

    let analysis = builder.build();

    let tokio = analysis.find_crate("tokio").unwrap();
    assert_eq!(tokio.purpose, "Async runtime for Rust");
    assert!(!tokio.apis_used.is_empty());
}

#[test]
fn test_ai_can_find_llm_dependencies() {
    // Scenario: AI asks "What LLM providers am I using?"
    let mut builder = DependencyAnalysisBuilder::new();
    builder.add_dashflow_crate(
        CrateDependency::new("dashflow-openai", "OpenAI")
            .with_category(DependencyCategory::LlmProvider),
    );
    builder.add_dashflow_crate(
        CrateDependency::new("dashflow-anthropic", "Anthropic")
            .with_category(DependencyCategory::LlmProvider),
    );

    let analysis = builder.build();

    let llm_crates = analysis.llm_provider_crates();
    assert_eq!(llm_crates.len(), 2);
    // AI sees: ["dashflow-openai", "dashflow-anthropic"]
}

#[test]
fn test_ai_can_get_dependency_summary() {
    // Scenario: AI asks "What are my dependencies?" (summary)
    let mut builder = DependencyAnalysisBuilder::new().dashflow_version("1.11.2");
    builder.add_dashflow_crate(CrateDependency::new("dashflow", "Core"));
    builder.add_dashflow_crate(CrateDependency::new("dashflow-openai", "OpenAI"));
    builder.add_external_crate(CrateDependency::new("tokio", "Runtime"));
    builder.add_external_crate(CrateDependency::new("serde", "Serialization"));

    let analysis = builder.build();
    let summary = analysis.summary();

    // AI gets quick summary
    assert!(summary.contains("4 total"));
    assert!(summary.contains("2 DashFlow"));
    assert!(summary.contains("2 external"));
}

#[test]
fn test_ai_can_parse_project_cargo_toml() {
    // Scenario: AI parses the project's Cargo.toml to understand dependencies
    let toml = r#"
[package]
name = "my-agent"
version = "0.1.0"

[dependencies]
dashflow = "1.11.2"
dashflow-openai = { version = "1.11.2", features = ["streaming"] }
dashflow-shell-tool = "1.11.2"
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }

[dev-dependencies]
mockall = "0.11"
"#;

    let analysis = parse_cargo_toml(toml);

    // AI understands the project's dependency stack
    assert_eq!(analysis.dashflow_version, "1.11.2");
    assert_eq!(analysis.dashflow_crates.len(), 3);
    assert_eq!(analysis.external_crates.len(), 3); // tokio, serde, mockall

    // AI can query specific crates
    assert!(analysis.uses_crate("dashflow-openai"));
    assert!(analysis.uses_crate("tokio"));

    // AI sees features
    let tokio = analysis.find_crate("tokio").unwrap();
    assert!(tokio.features.contains(&"full".to_string()));
}

#[test]
fn test_complete_dependency_analysis_scenario() {
    // Complete scenario: Full dependency analysis like a real AI agent
    let mut arch_builder = AppArchitecture::builder()
        .graph_structure(ArchitectureGraphInfo::new("start"))
        .metadata(ArchitectureMetadata::new());

    // Add dependencies as the architecture knows them
    arch_builder.add_dependency(
        Dependency::new("dashflow", "Core orchestration framework")
            .with_version("1.11.2")
            .dashflow(),
    );
    arch_builder.add_dependency(
        Dependency::new("dashflow-openai", "OpenAI LLM provider")
            .with_version("1.11.2")
            .dashflow()
            .with_api("ChatOpenAI"),
    );
    arch_builder.add_dependency(
        Dependency::new("dashflow-shell-tool", "Shell execution tool")
            .with_version("1.11.2")
            .dashflow()
            .with_api("SafeShellTool"),
    );
    arch_builder.add_dependency(Dependency::new("tokio", "Async runtime").with_version("1.35.0"));
    arch_builder.add_dependency(Dependency::new("serde", "Serialization").with_version("1.0.0"));

    let arch = arch_builder.build();

    // AI performs dependency analysis
    let analysis = arch.dependency_analysis();

    // Verify complete analysis
    assert!(!analysis.dashflow_version.is_empty());
    assert_eq!(analysis.dashflow_crates.len(), 3);
    assert_eq!(analysis.external_crates.len(), 2);
    assert_eq!(analysis.total_crates(), 5);

    // AI can query by category
    let llm_crates = analysis.llm_provider_crates();
    assert_eq!(llm_crates.len(), 1);
    assert_eq!(llm_crates[0].name, "dashflow-openai");

    let tool_crates = analysis.tool_crates();
    assert_eq!(tool_crates.len(), 1);

    // AI can get summary
    let summary = analysis.summary();
    assert!(summary.contains("5 total"));

    // AI can serialize to JSON for reporting
    let json = analysis.to_json().unwrap();
    assert!(json.contains("dashflow_crates"));
    assert!(json.contains("external_crates"));
}

// -------------------------------------------------------------------------
// Execution Flow Documentation Tests
// -------------------------------------------------------------------------

#[test]
fn test_execution_flow_builder() {
    let flow = ExecutionFlow::builder("test-graph")
        .description("A simple test flow")
        .entry_point("start")
        .build();

    assert_eq!(flow.graph_id, "test-graph");
    assert_eq!(flow.flow_description, "A simple test flow");
    assert_eq!(flow.entry_point, "start");
}

#[test]
fn test_execution_flow_default_values() {
    let flow = ExecutionFlow::builder("test").build();

    assert_eq!(flow.entry_point, "start");
    assert_eq!(flow.flow_description, "No description available");
    assert!(flow.exit_points.is_empty());
    assert!(flow.decision_points.is_empty());
    assert!(flow.loop_structures.is_empty());
}

#[test]
fn test_execution_flow_with_exit_points() {
    let mut builder = ExecutionFlow::builder("test");
    builder.add_exit_point("end");
    builder.add_exit_point("error_handler");

    let flow = builder.build();

    assert_eq!(flow.exit_points.len(), 2);
    assert!(flow.exit_points.contains(&"end".to_string()));
    assert!(flow.exit_points.contains(&"error_handler".to_string()));
}

#[test]
fn test_execution_flow_summary() {
    let mut builder = ExecutionFlow::builder("agent-graph").entry_point("input");
    builder.add_exit_point("output");
    builder.add_decision_point(DecisionPoint::new("router", "has_tools"));
    builder.add_loop_structure(LoopStructure::new("agent_loop", "reasoning"));
    builder.add_linear_path(ExecutionPath::new("main_path"));

    let flow = builder.build();
    let summary = flow.summary();

    assert!(summary.contains("agent-graph"));
    assert!(summary.contains("1 paths"));
    assert!(summary.contains("1 decisions"));
    assert!(summary.contains("1 loops"));
}

#[test]
fn test_execution_flow_has_cycles() {
    let flow_without_cycles = ExecutionFlow::builder("test").build();
    assert!(!flow_without_cycles.has_cycles());

    let mut builder = ExecutionFlow::builder("test");
    builder.add_loop_structure(LoopStructure::new("loop", "start"));
    let flow_with_cycles = builder.build();
    assert!(flow_with_cycles.has_cycles());
}

#[test]
fn test_execution_flow_has_branching() {
    let flow_without_branching = ExecutionFlow::builder("test").build();
    assert!(!flow_without_branching.has_branching());

    let mut builder = ExecutionFlow::builder("test");
    builder.add_decision_point(DecisionPoint::new("node", "condition"));
    let flow_with_branching = builder.build();
    assert!(flow_with_branching.has_branching());
}

#[test]
fn test_execution_flow_complexity_score() {
    // Simple linear flow
    let simple = ExecutionFlow::builder("test").build();
    assert_eq!(simple.complexity_score(), 1); // base only

    // Flow with decisions
    let mut builder = ExecutionFlow::builder("test");
    builder.add_decision_point(DecisionPoint::new("a", "cond"));
    builder.add_decision_point(DecisionPoint::new("b", "cond"));
    let with_decisions = builder.build();
    assert_eq!(with_decisions.complexity_score(), 5); // 1 + 2*2

    // Flow with loops
    let mut builder = ExecutionFlow::builder("test");
    builder.add_loop_structure(LoopStructure::new("loop", "start"));
    let with_loop = builder.build();
    assert_eq!(with_loop.complexity_score(), 4); // 1 + 3
}

#[test]
fn test_execution_flow_complexity_description() {
    let simple = ExecutionFlow::builder("test").build();
    assert_eq!(simple.complexity_description(), "Simple (linear flow)");

    let mut builder = ExecutionFlow::builder("test");
    builder.add_decision_point(DecisionPoint::new("a", "cond"));
    builder.add_decision_point(DecisionPoint::new("b", "cond"));
    let moderate = builder.build();
    assert_eq!(
        moderate.complexity_description(),
        "Moderate (some branching)"
    );
}

#[test]
fn test_execution_flow_find_decision() {
    let mut builder = ExecutionFlow::builder("test");
    builder.add_decision_point(
        DecisionPoint::new("router", "has_tools").with_explanation("Route based on tool presence"),
    );

    let flow = builder.build();

    let found = flow.find_decision("router");
    assert!(found.is_some());
    assert_eq!(found.unwrap().condition, "has_tools");

    let not_found = flow.find_decision("nonexistent");
    assert!(not_found.is_none());
}

#[test]
fn test_execution_flow_loops_containing() {
    let mut builder = ExecutionFlow::builder("test");
    builder.add_loop_structure(
        LoopStructure::new("agent_loop", "reasoning").with_nodes(vec![
            "reasoning",
            "tools",
            "analysis",
        ]),
    );
    builder.add_loop_structure(
        LoopStructure::new("retry_loop", "retry").with_nodes(vec!["retry", "validate"]),
    );

    let flow = builder.build();

    let containing_reasoning = flow.loops_containing("reasoning");
    assert_eq!(containing_reasoning.len(), 1);
    assert_eq!(containing_reasoning[0].name, "agent_loop");

    let containing_none = flow.loops_containing("output");
    assert!(containing_none.is_empty());
}

#[test]
fn test_execution_flow_json_serialization() {
    let mut builder = ExecutionFlow::builder("test-graph")
        .description("Test flow")
        .entry_point("start");
    builder.add_exit_point("end");

    let flow = builder.build();
    let json = flow.to_json().unwrap();

    assert!(json.contains("test-graph"));
    assert!(json.contains("Test flow"));

    // Should be parseable
    let parsed: ExecutionFlow = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.graph_id, "test-graph");
}

#[test]
fn test_decision_point_creation() {
    let dp = DecisionPoint::new("router", "has_pending_tools()")
        .with_explanation("Decides whether to execute tools or return")
        .with_type(DecisionType::ToolSelection)
        .with_path(DecisionPath::new("tool_execution", "tools are pending"))
        .with_path(DecisionPath::new("output", "no tools pending"));

    assert_eq!(dp.node, "router");
    assert_eq!(dp.condition, "has_pending_tools()");
    assert_eq!(dp.decision_type, DecisionType::ToolSelection);
    assert_eq!(dp.paths.len(), 2);
}

#[test]
fn test_decision_point_path_count() {
    let dp = DecisionPoint::new("router", "condition").with_paths(vec![
        DecisionPath::new("a", "when a"),
        DecisionPath::new("b", "when b"),
        DecisionPath::new("c", "when c"),
    ]);

    assert_eq!(dp.path_count(), 3);
    assert!(!dp.is_binary());
}

#[test]
fn test_decision_point_is_binary() {
    let binary = DecisionPoint::new("router", "condition")
        .with_path(DecisionPath::new("yes", "true"))
        .with_path(DecisionPath::new("no", "false"));

    assert!(binary.is_binary());
}

#[test]
fn test_decision_path_creation() {
    let path = DecisionPath::new("tool_node", "has_tool_calls()").with_probability(0.7);

    assert_eq!(path.target, "tool_node");
    assert_eq!(path.when, "has_tool_calls()");
    assert_eq!(path.probability, Some(0.7));
}

#[test]
fn test_decision_type_display() {
    assert_eq!(format!("{}", DecisionType::Conditional), "Conditional");
    assert_eq!(format!("{}", DecisionType::ToolSelection), "Tool Selection");
    assert_eq!(format!("{}", DecisionType::LoopControl), "Loop Control");
    assert_eq!(format!("{}", DecisionType::ErrorHandling), "Error Handling");
    assert_eq!(format!("{}", DecisionType::HumanApproval), "Human Approval");
    assert_eq!(format!("{}", DecisionType::Parallel), "Parallel");
}

#[test]
fn test_decision_type_default() {
    let dt: DecisionType = Default::default();
    assert_eq!(dt, DecisionType::Conditional);
}

#[test]
fn test_loop_structure_creation() {
    let loop_struct = LoopStructure::new("agent_loop", "reasoning")
        .with_nodes(vec!["reasoning", "tools", "analysis"])
        .with_exit_condition("!should_continue() || max_iterations_reached()")
        .with_explanation("Agent reasoning loop for iterative tool use")
        .with_max_iterations(10)
        .with_type(LoopType::AgentLoop);

    assert_eq!(loop_struct.name, "agent_loop");
    assert_eq!(loop_struct.entry_node, "reasoning");
    assert_eq!(loop_struct.nodes_in_loop.len(), 3);
    assert_eq!(loop_struct.max_iterations, Some(10));
    assert_eq!(loop_struct.loop_type, LoopType::AgentLoop);
}

#[test]
fn test_loop_structure_contains() {
    let loop_struct = LoopStructure::new("test", "start").with_nodes(vec!["reasoning", "tools"]);

    assert!(loop_struct.contains("reasoning"));
    assert!(loop_struct.contains("tools"));
    assert!(!loop_struct.contains("output"));
}

#[test]
fn test_loop_type_display() {
    assert_eq!(format!("{}", LoopType::Iterative), "Iterative");
    assert_eq!(format!("{}", LoopType::AgentLoop), "Agent Loop");
    assert_eq!(format!("{}", LoopType::RetryLoop), "Retry Loop");
    assert_eq!(format!("{}", LoopType::RefinementLoop), "Refinement Loop");
    assert_eq!(format!("{}", LoopType::MapReduce), "Map-Reduce");
}

#[test]
fn test_loop_type_default() {
    let lt: LoopType = Default::default();
    assert_eq!(lt, LoopType::Iterative);
}

#[test]
fn test_execution_path_creation() {
    let path = ExecutionPath::new("main_flow")
        .with_nodes(vec!["input", "process", "output"])
        .with_description("The main execution path without tool calls")
        .main_path();

    assert_eq!(path.name, "main_flow");
    assert_eq!(path.nodes.len(), 3);
    assert!(path.is_main_path);
    assert!(!path.is_empty());
    assert_eq!(path.len(), 3);
}

#[test]
fn test_execution_path_empty() {
    let path = ExecutionPath::new("empty");
    assert!(path.is_empty());
    assert_eq!(path.len(), 0);
}

#[test]
fn test_execution_flow_metadata() {
    let metadata = ExecutionFlowMetadata::new()
        .with_source("GraphAnalysis")
        .with_counts(5, 7)
        .with_note("Analyzed on test");

    assert_eq!(metadata.source, Some("GraphAnalysis".to_string()));
    assert_eq!(metadata.node_count, 5);
    assert_eq!(metadata.edge_count, 7);
    assert_eq!(metadata.notes.len(), 1);
}

#[test]
fn test_generate_flow_description_simple() {
    let nodes = vec![
        "start".to_string(),
        "process".to_string(),
        "end".to_string(),
    ];
    let desc = generate_flow_description("start", &nodes, &[], &[]);

    assert!(desc.contains("start"));
    assert!(desc.contains("3 nodes"));
    assert!(desc.contains("simple linear"));
}

#[test]
fn test_generate_flow_description_with_decisions() {
    let nodes = vec!["start".to_string(), "router".to_string()];
    let decisions = vec![DecisionPoint::new("router", "condition")
        .with_explanation("Route based on condition")
        .with_path(DecisionPath::new("a", "when true"))
        .with_path(DecisionPath::new("b", "when false"))];

    let desc = generate_flow_description("start", &nodes, &decisions, &[]);

    assert!(desc.contains("Decision Points"));
    assert!(desc.contains("router"));
    assert!(desc.contains("branching"));
}

#[test]
fn test_generate_flow_description_with_loops() {
    let nodes = vec!["start".to_string(), "loop".to_string()];
    let loops = vec![LoopStructure::new("main_loop", "loop")
        .with_explanation("Iterates until done")
        .with_exit_condition("is_complete()")];

    let desc = generate_flow_description("start", &nodes, &[], &loops);

    assert!(desc.contains("Loop Structures"));
    assert!(desc.contains("main_loop"));
    assert!(desc.contains("iterative"));
}

#[test]
fn test_generate_flow_description_complex() {
    let nodes = vec!["start".to_string()];
    let decisions = vec![DecisionPoint::new("router", "cond").with_explanation("Route")];
    let loops = vec![LoopStructure::new("loop", "node")
        .with_explanation("Loop")
        .with_exit_condition("done")];

    let desc = generate_flow_description("start", &nodes, &decisions, &loops);

    assert!(desc.contains("complex (branching with loops)"));
}

// AI usage scenario tests

#[test]
fn test_ai_can_ask_how_do_i_work() {
    // Scenario: AI asks "How do I work?"
    let mut builder = ExecutionFlow::builder("claude-agent")
        .description("I am a coding agent that processes user requests iteratively")
        .entry_point("user_input");
    builder.add_exit_point("final_response");

    let flow = builder.build();

    // AI gets natural language description
    assert!(flow.flow_description.contains("coding agent"));
}

#[test]
fn test_ai_can_understand_decision_points() {
    // Scenario: AI asks "What decisions do I make?"
    let mut builder = ExecutionFlow::builder("agent");
    builder.add_decision_point(
        DecisionPoint::new("reasoning", "has_pending_tool_calls()")
            .with_explanation("Decide whether to execute tools or respond to user")
            .with_type(DecisionType::ToolSelection)
            .with_path(DecisionPath::new(
                "tool_execution",
                "tools need to be executed",
            ))
            .with_path(DecisionPath::new("output", "ready to respond")),
    );
    builder.add_decision_point(
        DecisionPoint::new("tool_result", "should_continue()")
            .with_explanation("Decide whether to continue reasoning or finish")
            .with_type(DecisionType::LoopControl)
            .with_path(DecisionPath::new("reasoning", "need more iterations"))
            .with_path(DecisionPath::new("output", "task complete")),
    );

    let flow = builder.build();

    // AI sees all decision points
    assert_eq!(flow.decision_points.len(), 2);

    // AI can query specific decision
    let reasoning_decision = flow.find_decision("reasoning").unwrap();
    assert!(reasoning_decision.explanation.contains("execute tools"));
}

#[test]
fn test_ai_can_understand_loops() {
    // Scenario: AI asks "Do I have any iterative patterns?"
    let mut builder = ExecutionFlow::builder("agent");
    builder.add_loop_structure(
        LoopStructure::new("agent_loop", "reasoning")
            .with_nodes(vec!["reasoning", "tool_execution", "tool_result"])
            .with_exit_condition("!should_continue() || iteration > max_iterations")
            .with_explanation("Iterative reasoning loop for complex tasks")
            .with_type(LoopType::AgentLoop)
            .with_max_iterations(10),
    );

    let flow = builder.build();

    // AI knows it has loops
    assert!(flow.has_cycles());

    // AI can get loop details
    let agent_loop = &flow.loop_structures[0];
    assert_eq!(agent_loop.loop_type, LoopType::AgentLoop);
    assert_eq!(agent_loop.max_iterations, Some(10));
    assert!(agent_loop.contains("reasoning"));
}

#[test]
fn test_ai_can_get_execution_summary() {
    // Scenario: AI asks "Give me a quick summary of my execution"
    let mut builder = ExecutionFlow::builder("claude-code-agent").entry_point("user_input");
    builder.add_exit_point("response");
    builder.add_decision_point(DecisionPoint::new("router", "condition"));
    builder.add_loop_structure(LoopStructure::new("loop", "reasoning"));
    builder.add_linear_path(
        ExecutionPath::new("main")
            .with_nodes(vec!["user_input", "reasoning", "response"])
            .main_path(),
    );

    let flow = builder.build();
    let summary = flow.summary();

    // AI gets quick overview
    assert!(summary.contains("claude-code-agent"));
    assert!(summary.contains("1 paths"));
    assert!(summary.contains("1 decisions"));
    assert!(summary.contains("1 loops"));
}

#[test]
fn test_ai_can_assess_complexity() {
    // Scenario: AI asks "How complex am I?"
    let mut builder = ExecutionFlow::builder("agent");
    builder.add_decision_point(DecisionPoint::new("a", "cond"));
    builder.add_decision_point(DecisionPoint::new("b", "cond"));
    builder.add_loop_structure(LoopStructure::new("loop", "c"));

    let flow = builder.build();

    // AI gets complexity assessment
    let score = flow.complexity_score();
    let desc = flow.complexity_description();

    assert!(score > 5);
    assert!(desc.contains("Complex") || desc.contains("Moderate"));
}

#[test]
fn test_complete_execution_flow_scenario() {
    // Complete scenario: Full execution flow like a real AI agent
    let mut builder = ExecutionFlow::builder("claude-code-agent")
        .description(
            "I am Claude Code, a coding agent that:\n\
                 1. Receives user input\n\
                 2. Reasons about the task using GPT-4\n\
                 3. Executes tools if needed (shell, file, search)\n\
                 4. Analyzes results and iterates\n\
                 5. Provides final response",
        )
        .entry_point("user_input")
        .metadata(ExecutionFlowMetadata::new().with_counts(6, 8));

    builder.add_exit_point("response");
    builder.add_exit_point("error_handler");

    // Decision points
    builder.add_decision_point(
        DecisionPoint::new("reasoning", "has_tool_calls()")
            .with_explanation("After reasoning, decide if tools are needed")
            .with_type(DecisionType::ToolSelection)
            .with_path(DecisionPath::new("tool_execution", "tools requested").with_probability(0.7))
            .with_path(DecisionPath::new("response", "ready to respond").with_probability(0.3)),
    );

    builder.add_decision_point(
        DecisionPoint::new("result_analysis", "should_continue()")
            .with_explanation("After tool results, decide if more work needed")
            .with_type(DecisionType::LoopControl)
            .with_path(DecisionPath::new("reasoning", "needs more iterations"))
            .with_path(DecisionPath::new("response", "task complete")),
    );

    // Loop structure
    builder.add_loop_structure(
        LoopStructure::new("agent_loop", "reasoning")
            .with_nodes(vec![
                "reasoning".to_string(),
                "tool_execution".to_string(),
                "result_analysis".to_string(),
            ])
            .with_exit_condition("!should_continue() || iterations >= 10")
            .with_explanation("Iterative reasoning-action loop for complex tasks")
            .with_type(LoopType::AgentLoop)
            .with_max_iterations(10),
    );

    // Main execution path
    builder.add_linear_path(
        ExecutionPath::new("simple_response")
            .with_nodes(vec!["user_input", "reasoning", "response"])
            .with_description("Direct response without tool execution")
            .main_path(),
    );

    builder.add_linear_path(
        ExecutionPath::new("tool_execution_path")
            .with_nodes(vec![
                "user_input",
                "reasoning",
                "tool_execution",
                "result_analysis",
                "response",
            ])
            .with_description("Path with tool execution"),
    );

    let flow = builder.build();

    // Verify complete flow
    assert_eq!(flow.graph_id, "claude-code-agent");
    assert_eq!(flow.entry_point, "user_input");
    assert_eq!(flow.exit_points.len(), 2);
    assert_eq!(flow.decision_points.len(), 2);
    assert_eq!(flow.loop_structures.len(), 1);
    assert_eq!(flow.linear_paths.len(), 2);

    // AI can query specific parts
    assert!(flow.has_cycles());
    assert!(flow.has_branching());

    let tool_decision = flow.find_decision("reasoning").unwrap();
    assert!(tool_decision.is_binary());

    let loops = flow.loops_containing("reasoning");
    assert_eq!(loops.len(), 1);

    // Complexity assessment
    assert!(flow.complexity_score() > 8);
    assert_eq!(
        flow.complexity_description(),
        "Complex (multiple paths and loops)"
    );

    // JSON serialization
    let json = flow.to_json().unwrap();
    assert!(json.contains("claude-code-agent"));
    assert!(json.contains("decision_points"));
    assert!(json.contains("loop_structures"));
}

// -------------------------------------------------------------------------
// Node Purpose Explanation Tests
// -------------------------------------------------------------------------

#[test]
fn test_node_purpose_creation() {
    let purpose = NodePurpose::new("reasoning", "Processes user input and generates reasoning");
    assert_eq!(purpose.node_name, "reasoning");
    assert_eq!(
        purpose.purpose,
        "Processes user input and generates reasoning"
    );
    assert!(purpose.inputs.is_empty());
    assert!(purpose.outputs.is_empty());
    assert!(purpose.apis_used.is_empty());
    assert!(purpose.external_calls.is_empty());
    assert_eq!(purpose.node_type, NodeType::Processing);
}

#[test]
fn test_state_field_usage() {
    let field = StateFieldUsage::new("messages", "Conversation history")
        .with_type("Vec<Message>")
        .required();

    assert_eq!(field.field_name, "messages");
    assert_eq!(field.description, "Conversation history");
    assert_eq!(field.field_type, Some("Vec<Message>".to_string()));
    assert!(field.required);
    assert!(field.default_value.is_none());
}

#[test]
fn test_state_field_with_default() {
    let field = StateFieldUsage::new("temperature", "Model temperature")
        .with_type("f64")
        .with_default("0.7");

    assert_eq!(field.default_value, Some("0.7".to_string()));
    assert!(!field.required);
}

#[test]
fn test_api_usage() {
    let api = ApiUsage::new("ChatOpenAI::invoke", "Generates LLM responses")
        .with_module("dashflow_openai")
        .critical();

    assert_eq!(api.api_name, "ChatOpenAI::invoke");
    assert_eq!(api.usage_description, "Generates LLM responses");
    assert_eq!(api.module, Some("dashflow_openai".to_string()));
    assert!(api.is_critical);
}

#[test]
fn test_external_call_llm() {
    let call = ExternalCall::new(
        "OpenAI API",
        "Makes completion request",
        ExternalCallType::LlmApi,
    )
    .with_endpoint("https://api.openai.com/v1/chat/completions")
    .with_latency("500ms - 10s");

    assert_eq!(call.service_name, "OpenAI API");
    assert_eq!(call.call_type, ExternalCallType::LlmApi);
    assert!(call.may_fail);
    assert_eq!(
        call.endpoint,
        Some("https://api.openai.com/v1/chat/completions".to_string())
    );
    assert_eq!(call.latency, Some("500ms - 10s".to_string()));
}

#[test]
fn test_external_call_tool() {
    let call = ExternalCall::new(
        "Shell Executor",
        "Runs shell commands",
        ExternalCallType::ToolExecution,
    )
    .reliable();

    assert_eq!(call.call_type, ExternalCallType::ToolExecution);
    assert!(!call.may_fail);
}

#[test]
fn test_external_call_type_display() {
    assert_eq!(format!("{}", ExternalCallType::LlmApi), "LLM API");
    assert_eq!(
        format!("{}", ExternalCallType::ToolExecution),
        "Tool Execution"
    );
    assert_eq!(format!("{}", ExternalCallType::HttpRequest), "HTTP Request");
    assert_eq!(
        format!("{}", ExternalCallType::DatabaseQuery),
        "Database Query"
    );
    assert_eq!(
        format!("{}", ExternalCallType::VectorStoreOp),
        "Vector Store"
    );
    assert_eq!(
        format!("{}", ExternalCallType::MessageQueue),
        "Message Queue"
    );
    assert_eq!(format!("{}", ExternalCallType::FileSystem), "File System");
    assert_eq!(format!("{}", ExternalCallType::ExternalApi), "External API");
    assert_eq!(format!("{}", ExternalCallType::Other), "Other");
}

#[test]
fn test_node_type_display() {
    assert_eq!(format!("{}", NodeType::EntryPoint), "Entry Point");
    assert_eq!(format!("{}", NodeType::ExitPoint), "Exit Point");
    assert_eq!(format!("{}", NodeType::Processing), "Processing");
    assert_eq!(format!("{}", NodeType::Router), "Router");
    assert_eq!(format!("{}", NodeType::LlmNode), "LLM Node");
    assert_eq!(format!("{}", NodeType::ToolNode), "Tool Node");
    assert_eq!(format!("{}", NodeType::Transform), "Transform");
    assert_eq!(format!("{}", NodeType::HumanInLoop), "Human-in-Loop");
    assert_eq!(format!("{}", NodeType::Aggregator), "Aggregator");
    assert_eq!(format!("{}", NodeType::Validator), "Validator");
}

#[test]
fn test_node_purpose_builder() {
    let mut builder = NodePurpose::builder("reasoning")
        .purpose("AI reasoning and decision making")
        .node_type(NodeType::LlmNode);

    builder.add_input(
        StateFieldUsage::new("messages", "Conversation history")
            .with_type("Vec<Message>")
            .required(),
    );
    builder.add_output(StateFieldUsage::new("response", "Generated response").with_type("String"));
    builder.add_api(ApiUsage::new("ChatOpenAI::invoke", "Generates responses").critical());
    builder.add_external_call(ExternalCall::new(
        "OpenAI",
        "LLM call",
        ExternalCallType::LlmApi,
    ));

    let purpose = builder.build();

    assert_eq!(purpose.node_name, "reasoning");
    assert_eq!(purpose.purpose, "AI reasoning and decision making");
    assert_eq!(purpose.node_type, NodeType::LlmNode);
    assert_eq!(purpose.inputs.len(), 1);
    assert_eq!(purpose.outputs.len(), 1);
    assert_eq!(purpose.apis_used.len(), 1);
    assert_eq!(purpose.external_calls.len(), 1);
}

#[test]
fn test_node_purpose_queries() {
    let mut builder = NodePurpose::builder("tool_executor")
        .purpose("Executes tools")
        .node_type(NodeType::ToolNode);

    builder.add_input(StateFieldUsage::new("tool_calls", "Tool calls to execute"));
    builder.add_output(StateFieldUsage::new("tool_results", "Results from tools"));
    builder.add_api(ApiUsage::new("ToolExecutor::run", "Runs tools"));
    builder.add_external_call(ExternalCall::new(
        "Shell",
        "Shell execution",
        ExternalCallType::ToolExecution,
    ));
    builder.add_external_call(ExternalCall::new(
        "OpenAI",
        "LLM call",
        ExternalCallType::LlmApi,
    ));

    let purpose = builder.build();

    // Query methods
    assert!(purpose.has_inputs());
    assert!(purpose.has_outputs());
    assert!(purpose.has_external_calls());
    assert!(purpose.uses_dashflow_apis());

    // Field lookup
    assert!(purpose.reads_field("tool_calls"));
    assert!(!purpose.reads_field("messages"));
    assert!(purpose.writes_field("tool_results"));
    assert!(!purpose.writes_field("response"));

    // API lookup
    assert!(purpose.uses_api("ToolExecutor"));
    assert!(!purpose.uses_api("ChatOpenAI"));

    // Service lookup
    assert!(purpose.calls_service("Shell"));
    assert!(purpose.calls_service("openai"));

    // Filtered calls
    assert_eq!(purpose.llm_calls().len(), 1);
    assert_eq!(purpose.tool_calls().len(), 1);
    assert!(purpose.service_calls().is_empty());
}

#[test]
fn test_node_purpose_get_input_output() {
    let mut builder = NodePurpose::builder("node");
    builder.add_input(StateFieldUsage::new("field_a", "Input A"));
    builder.add_input(StateFieldUsage::new("field_b", "Input B"));
    builder.add_output(StateFieldUsage::new("field_c", "Output C"));

    let purpose = builder.build();

    let input_a = purpose.get_input("field_a");
    assert!(input_a.is_some());
    assert_eq!(input_a.unwrap().description, "Input A");

    let input_b = purpose.get_input("field_b");
    assert!(input_b.is_some());

    let missing = purpose.get_input("nonexistent");
    assert!(missing.is_none());

    let output_c = purpose.get_output("field_c");
    assert!(output_c.is_some());
    assert_eq!(output_c.unwrap().description, "Output C");
}

#[test]
fn test_node_purpose_summary() {
    let mut builder = NodePurpose::builder("reasoning").node_type(NodeType::LlmNode);
    builder.add_input(StateFieldUsage::new("messages", "Messages"));
    builder.add_input(StateFieldUsage::new("context", "Context"));
    builder.add_output(StateFieldUsage::new("response", "Response"));
    builder.add_api(ApiUsage::new("ChatOpenAI", "Chat"));
    builder.add_external_call(ExternalCall::new("OpenAI", "LLM", ExternalCallType::LlmApi));

    let purpose = builder.build();
    let summary = purpose.summary();

    assert!(summary.contains("reasoning"));
    assert!(summary.contains("LLM Node"));
    assert!(summary.contains("2 inputs"));
    assert!(summary.contains("1 outputs"));
    assert!(summary.contains("1 APIs"));
    assert!(summary.contains("1 external calls"));
}

#[test]
fn test_node_purpose_explain() {
    let mut builder = NodePurpose::builder("reasoning")
        .purpose("Analyzes user input and generates responses")
        .node_type(NodeType::LlmNode);

    builder.add_input(StateFieldUsage::new("messages", "Conversation history").required());
    builder.add_output(StateFieldUsage::new("response", "Generated response"));
    builder.add_api(ApiUsage::new("ChatOpenAI::invoke", "Makes LLM calls"));
    builder.add_external_call(ExternalCall::new(
        "OpenAI API",
        "Chat completion",
        ExternalCallType::LlmApi,
    ));

    let purpose = builder.build();
    let explanation = purpose.explain();

    assert!(explanation.contains("**reasoning**"));
    assert!(explanation.contains("Analyzes user input"));
    assert!(explanation.contains("Type: LLM Node"));
    assert!(explanation.contains("Reads from state:"));
    assert!(explanation.contains("messages"));
    assert!(explanation.contains("(required)"));
    assert!(explanation.contains("Writes to state:"));
    assert!(explanation.contains("response"));
    assert!(explanation.contains("DashFlow APIs used:"));
    assert!(explanation.contains("ChatOpenAI::invoke"));
    assert!(explanation.contains("External services called:"));
    assert!(explanation.contains("OpenAI API"));
}

#[test]
fn test_node_purpose_json() {
    let purpose = NodePurpose::new("test_node", "Test purpose");
    let json = purpose.to_json().unwrap();

    assert!(json.contains("test_node"));
    assert!(json.contains("Test purpose"));
    assert!(json.contains("node_type"));
}

#[test]
fn test_node_purpose_metadata() {
    let metadata = NodePurposeMetadata::new()
        .with_source("src/graph.rs", 42)
        .with_author("AI Team")
        .with_note("This node handles core logic")
        .with_tag("llm")
        .with_tag("reasoning");

    assert_eq!(metadata.source_file, Some("src/graph.rs".to_string()));
    assert_eq!(metadata.source_line, Some(42));
    assert_eq!(metadata.author, Some("AI Team".to_string()));
    assert_eq!(metadata.notes.len(), 1);
    assert_eq!(metadata.tags.len(), 2);
}

#[test]
fn test_node_purpose_collection_creation() {
    let collection = NodePurposeCollection::new();
    assert!(collection.is_empty());
    assert_eq!(collection.len(), 0);
    assert!(collection.graph_id.is_none());
}

#[test]
fn test_node_purpose_collection_for_graph() {
    let collection = NodePurposeCollection::for_graph("my-agent");
    assert!(collection.is_empty());
    assert_eq!(collection.graph_id, Some("my-agent".to_string()));
}

#[test]
fn test_node_purpose_collection_add_get() {
    let mut collection = NodePurposeCollection::new();

    collection.add(NodePurpose::new("node_a", "Purpose A"));
    collection.add(NodePurpose::new("node_b", "Purpose B"));

    assert_eq!(collection.len(), 2);
    assert!(!collection.is_empty());
    assert!(collection.contains("node_a"));
    assert!(collection.contains("node_b"));
    assert!(!collection.contains("node_c"));

    let node_a = collection.get("node_a");
    assert!(node_a.is_some());
    assert_eq!(node_a.unwrap().purpose, "Purpose A");
}

#[test]
fn test_node_purpose_collection_node_names() {
    let mut collection = NodePurposeCollection::new();
    collection.add(NodePurpose::new("alpha", "Alpha"));
    collection.add(NodePurpose::new("beta", "Beta"));
    collection.add(NodePurpose::new("gamma", "Gamma"));

    let names = collection.node_names();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"alpha"));
    assert!(names.contains(&"beta"));
    assert!(names.contains(&"gamma"));
}

#[test]
fn test_node_purpose_collection_nodes_with_external_calls() {
    let mut collection = NodePurposeCollection::new();

    // Node without external calls
    collection.add(NodePurpose::new("simple", "Simple processing"));

    // Node with external calls
    let mut builder = NodePurpose::builder("llm_node");
    builder.add_external_call(ExternalCall::new("OpenAI", "LLM", ExternalCallType::LlmApi));
    collection.add(builder.build());

    let with_calls = collection.nodes_with_external_calls();
    assert_eq!(with_calls.len(), 1);
    assert_eq!(with_calls[0].node_name, "llm_node");
}

#[test]
fn test_node_purpose_collection_nodes_by_type() {
    let mut collection = NodePurposeCollection::new();

    collection.add({
        NodePurpose::builder("entry")
            .node_type(NodeType::EntryPoint)
            .build()
    });
    collection.add({
        NodePurpose::builder("reasoning")
            .node_type(NodeType::LlmNode)
            .build()
    });
    collection.add({
        NodePurpose::builder("tool_exec")
            .node_type(NodeType::ToolNode)
            .build()
    });
    collection.add({
        NodePurpose::builder("router")
            .node_type(NodeType::Router)
            .build()
    });
    collection.add({
        NodePurpose::builder("exit")
            .node_type(NodeType::ExitPoint)
            .build()
    });

    assert_eq!(collection.nodes_by_type(NodeType::EntryPoint).len(), 1);
    assert_eq!(collection.llm_nodes().len(), 1);
    assert_eq!(collection.tool_nodes().len(), 1);
    assert_eq!(collection.router_nodes().len(), 1);
    assert_eq!(collection.nodes_by_type(NodeType::ExitPoint).len(), 1);
}

#[test]
fn test_node_purpose_collection_nodes_reading_field() {
    let mut collection = NodePurposeCollection::new();

    let mut builder1 = NodePurpose::builder("node1");
    builder1.add_input(StateFieldUsage::new("messages", "Messages"));
    collection.add(builder1.build());

    let mut builder2 = NodePurpose::builder("node2");
    builder2.add_input(StateFieldUsage::new("messages", "Messages"));
    builder2.add_input(StateFieldUsage::new("context", "Context"));
    collection.add(builder2.build());

    let mut builder3 = NodePurpose::builder("node3");
    builder3.add_input(StateFieldUsage::new("context", "Context"));
    collection.add(builder3.build());

    let reading_messages = collection.nodes_reading_field("messages");
    assert_eq!(reading_messages.len(), 2);

    let reading_context = collection.nodes_reading_field("context");
    assert_eq!(reading_context.len(), 2);

    let reading_nonexistent = collection.nodes_reading_field("nonexistent");
    assert!(reading_nonexistent.is_empty());
}

#[test]
fn test_node_purpose_collection_nodes_writing_field() {
    let mut collection = NodePurposeCollection::new();

    let mut builder1 = NodePurpose::builder("producer");
    builder1.add_output(StateFieldUsage::new("result", "Result"));
    collection.add(builder1.build());

    let mut builder2 = NodePurpose::builder("consumer");
    builder2.add_input(StateFieldUsage::new("result", "Result"));
    collection.add(builder2.build());

    let writing_result = collection.nodes_writing_field("result");
    assert_eq!(writing_result.len(), 1);
    assert_eq!(writing_result[0].node_name, "producer");
}

#[test]
fn test_node_purpose_collection_nodes_using_api() {
    let mut collection = NodePurposeCollection::new();

    let mut builder1 = NodePurpose::builder("node1");
    builder1.add_api(ApiUsage::new("ChatOpenAI::invoke", "LLM"));
    collection.add(builder1.build());

    let mut builder2 = NodePurpose::builder("node2");
    builder2.add_api(ApiUsage::new("ToolExecutor::run", "Tools"));
    collection.add(builder2.build());

    let using_chat = collection.nodes_using_api("ChatOpenAI");
    assert_eq!(using_chat.len(), 1);

    let using_tool = collection.nodes_using_api("ToolExecutor");
    assert_eq!(using_tool.len(), 1);

    let using_none = collection.nodes_using_api("NonexistentAPI");
    assert!(using_none.is_empty());
}

#[test]
fn test_node_purpose_collection_nodes_calling_service() {
    let mut collection = NodePurposeCollection::new();

    let mut builder1 = NodePurpose::builder("llm_node");
    builder1.add_external_call(ExternalCall::new("OpenAI", "LLM", ExternalCallType::LlmApi));
    collection.add(builder1.build());

    let mut builder2 = NodePurpose::builder("db_node");
    builder2.add_external_call(ExternalCall::new(
        "PostgreSQL",
        "DB",
        ExternalCallType::DatabaseQuery,
    ));
    collection.add(builder2.build());

    let calling_openai = collection.nodes_calling_service("OpenAI");
    assert_eq!(calling_openai.len(), 1);

    let calling_postgres = collection.nodes_calling_service("PostgreSQL");
    assert_eq!(calling_postgres.len(), 1);
}

#[test]
fn test_node_purpose_collection_summary() {
    let mut collection = NodePurposeCollection::new();

    collection.add(
        NodePurpose::builder("entry")
            .node_type(NodeType::EntryPoint)
            .build(),
    );

    let mut llm_builder = NodePurpose::builder("reasoning").node_type(NodeType::LlmNode);
    llm_builder.add_external_call(ExternalCall::new("OpenAI", "LLM", ExternalCallType::LlmApi));
    collection.add(llm_builder.build());

    collection.add(
        NodePurpose::builder("tool")
            .node_type(NodeType::ToolNode)
            .build(),
    );
    collection.add(
        NodePurpose::builder("router")
            .node_type(NodeType::Router)
            .build(),
    );

    let summary = collection.summary();
    assert!(summary.contains("4 total"));
    assert!(summary.contains("1 LLM"));
    assert!(summary.contains("1 tool"));
    assert!(summary.contains("1 router"));
    assert!(summary.contains("1 with external calls"));
}

#[test]
fn test_node_purpose_collection_explain_all() {
    let mut collection = NodePurposeCollection::for_graph("test-graph");

    collection.add(NodePurpose::new("node_a", "Does A"));
    collection.add(NodePurpose::new("node_b", "Does B"));

    let explanation = collection.explain_all();

    assert!(explanation.contains("# Node Explanations for 'test-graph'"));
    assert!(explanation.contains("node_a") || explanation.contains("node_b"));
    assert!(explanation.contains("---"));
}

#[test]
fn test_node_purpose_collection_json() {
    let mut collection = NodePurposeCollection::for_graph("my-graph");
    collection.add(NodePurpose::new("node", "Purpose"));

    let json = collection.to_json().unwrap();
    assert!(json.contains("my-graph"));
    assert!(json.contains("node"));
}

#[test]
fn test_node_purpose_collection_metadata() {
    let metadata = NodePurposeCollectionMetadata::new()
        .with_timestamp("2024-01-15T10:30:00Z")
        .with_source("static analysis")
        .with_note("Generated automatically");

    assert_eq!(
        metadata.generated_at,
        Some("2024-01-15T10:30:00Z".to_string())
    );
    assert_eq!(metadata.source, Some("static analysis".to_string()));
    assert_eq!(metadata.notes.len(), 1);
}

#[test]
fn test_infer_node_type_by_name() {
    // Entry points
    assert_eq!(
        infer_node_type("entry", false, false, false),
        NodeType::EntryPoint
    );
    assert_eq!(
        infer_node_type("start_node", false, false, false),
        NodeType::EntryPoint
    );
    assert_eq!(
        infer_node_type("input_handler", false, false, false),
        NodeType::EntryPoint
    );

    // Exit points
    assert_eq!(
        infer_node_type("exit", false, false, false),
        NodeType::ExitPoint
    );
    assert_eq!(
        infer_node_type("end_node", false, false, false),
        NodeType::ExitPoint
    );
    assert_eq!(
        infer_node_type("output_handler", false, false, false),
        NodeType::ExitPoint
    );
    assert_eq!(
        infer_node_type("response_builder", false, false, false),
        NodeType::ExitPoint
    );

    // Routers
    assert_eq!(
        infer_node_type("route_handler", false, false, false),
        NodeType::Router
    );
    assert_eq!(
        infer_node_type("dispatcher", false, false, false),
        NodeType::Router
    );

    // Human-in-loop
    assert_eq!(
        infer_node_type("human_review", false, false, false),
        NodeType::HumanInLoop
    );
    assert_eq!(
        infer_node_type("approval_step", false, false, false),
        NodeType::HumanInLoop
    );

    // Validators
    assert_eq!(
        infer_node_type("validator", false, false, false),
        NodeType::Validator
    );
    assert_eq!(
        infer_node_type("guard_check", false, false, false),
        NodeType::Validator
    );

    // Aggregators
    assert_eq!(
        infer_node_type("aggregator", false, false, false),
        NodeType::Aggregator
    );
    assert_eq!(
        infer_node_type("merge_results", false, false, false),
        NodeType::Aggregator
    );
    assert_eq!(
        infer_node_type("combine_data", false, false, false),
        NodeType::Aggregator
    );

    // Transforms
    assert_eq!(
        infer_node_type("transform_data", false, false, false),
        NodeType::Transform
    );
    assert_eq!(
        infer_node_type("converter", false, false, false),
        NodeType::Transform
    );
    assert_eq!(
        infer_node_type("parser", false, false, false),
        NodeType::Transform
    );
}

#[test]
fn test_infer_node_type_by_behavior() {
    // Routing behavior takes precedence
    assert_eq!(
        infer_node_type("processor", false, false, true),
        NodeType::Router
    );

    // LLM calls
    assert_eq!(
        infer_node_type("processor", true, false, false),
        NodeType::LlmNode
    );

    // Tool calls
    assert_eq!(
        infer_node_type("processor", false, true, false),
        NodeType::ToolNode
    );

    // No behavior = Processing
    assert_eq!(
        infer_node_type("processor", false, false, false),
        NodeType::Processing
    );
}

#[test]
fn test_infer_node_type_name_takes_precedence() {
    // Even with LLM calls, "router" name should return Router
    assert_eq!(
        infer_node_type("router", true, true, false),
        NodeType::Router
    );

    // Even with tool calls, "entry" name should return EntryPoint
    assert_eq!(
        infer_node_type("entry_point", false, true, false),
        NodeType::EntryPoint
    );
}

#[test]
fn test_comprehensive_agent_node_purposes() {
    // Build a complete agent node purpose collection (like Claude Code)
    let mut collection = NodePurposeCollection::for_graph("claude-code-agent");

    // Entry node
    let mut entry_builder = NodePurpose::builder("user_input")
        .purpose("Receives and validates user messages")
        .node_type(NodeType::EntryPoint);
    entry_builder.add_output(
        StateFieldUsage::new("messages", "Updated message history").with_type("Vec<Message>"),
    );
    collection.add(entry_builder.build());

    // Reasoning node (LLM)
    let mut reasoning_builder = NodePurpose::builder("reasoning")
        .purpose("Analyzes user intent and decides on actions")
        .node_type(NodeType::LlmNode);
    reasoning_builder.add_input(
        StateFieldUsage::new("messages", "Conversation history")
            .with_type("Vec<Message>")
            .required(),
    );
    reasoning_builder
        .add_input(StateFieldUsage::new("context", "Additional context").with_type("String"));
    reasoning_builder.add_output(
        StateFieldUsage::new("response", "LLM response with potential tool calls")
            .with_type("LlmResponse"),
    );
    reasoning_builder.add_api(
        ApiUsage::new("ChatOpenAI::invoke", "Makes LLM inference requests")
            .with_module("dashflow_openai")
            .critical(),
    );
    reasoning_builder.add_external_call(
        ExternalCall::new(
            "OpenAI API",
            "Chat completion request",
            ExternalCallType::LlmApi,
        )
        .with_endpoint("https://api.openai.com/v1/chat/completions")
        .with_latency("500ms - 30s"),
    );
    collection.add(reasoning_builder.build());

    // Router node
    let mut router_builder = NodePurpose::builder("action_router")
        .purpose("Routes to tool execution or response based on LLM output")
        .node_type(NodeType::Router);
    router_builder.add_input(StateFieldUsage::new("response", "LLM response").required());
    router_builder.add_output(
        StateFieldUsage::new("next_action", "Decided action (execute_tools or respond)")
            .with_type("Action"),
    );
    collection.add(router_builder.build());

    // Tool execution node
    let mut tool_builder = NodePurpose::builder("tool_executor")
        .purpose("Executes requested tools and captures results")
        .node_type(NodeType::ToolNode);
    tool_builder.add_input(
        StateFieldUsage::new("tool_calls", "Tool calls from LLM")
            .with_type("Vec<ToolCall>")
            .required(),
    );
    tool_builder.add_output(
        StateFieldUsage::new("tool_results", "Results from tool execution")
            .with_type("Vec<ToolResult>"),
    );
    tool_builder.add_api(
        ApiUsage::new("ToolExecutor::execute", "Runs tools in sandbox")
            .with_module("dashflow_tools"),
    );
    tool_builder.add_external_call(ExternalCall::new(
        "Shell Executor",
        "Runs shell commands",
        ExternalCallType::ToolExecution,
    ));
    tool_builder.add_external_call(
        ExternalCall::new(
            "File System",
            "Reads/writes files",
            ExternalCallType::FileSystem,
        )
        .reliable(),
    );
    collection.add(tool_builder.build());

    // Response node
    let mut response_builder = NodePurpose::builder("response_formatter")
        .purpose("Formats and sends response to user")
        .node_type(NodeType::ExitPoint);
    response_builder.add_input(StateFieldUsage::new("response", "Response content").required());
    response_builder.add_output(
        StateFieldUsage::new("formatted_output", "User-facing output").with_type("String"),
    );
    collection.add(response_builder.build());

    // Verify collection
    assert_eq!(collection.len(), 5);
    assert_eq!(collection.graph_id, Some("claude-code-agent".to_string()));

    // Query by type
    assert_eq!(collection.nodes_by_type(NodeType::EntryPoint).len(), 1);
    assert_eq!(collection.llm_nodes().len(), 1);
    assert_eq!(collection.tool_nodes().len(), 1);
    assert_eq!(collection.router_nodes().len(), 1);
    assert_eq!(collection.nodes_by_type(NodeType::ExitPoint).len(), 1);

    // Query by field
    let nodes_reading_messages = collection.nodes_reading_field("messages");
    assert_eq!(nodes_reading_messages.len(), 1);
    assert_eq!(nodes_reading_messages[0].node_name, "reasoning");

    let nodes_writing_response = collection.nodes_writing_field("response");
    assert_eq!(nodes_writing_response.len(), 1);

    // Query by service
    let nodes_calling_openai = collection.nodes_calling_service("OpenAI");
    assert_eq!(nodes_calling_openai.len(), 1);

    // External calls summary
    let with_external = collection.nodes_with_external_calls();
    assert_eq!(with_external.len(), 2); // reasoning and tool_executor

    // Generate full explanation
    let explanation = collection.explain_all();
    assert!(explanation.contains("claude-code-agent"));
    assert!(explanation.contains("reasoning"));
    assert!(explanation.contains("tool_executor"));

    // JSON export
    let json = collection.to_json().unwrap();
    assert!(json.contains("claude-code-agent"));
    assert!(json.contains("reasoning"));
    assert!(json.contains("ChatOpenAI::invoke"));
}

#[test]
fn test_node_purpose_collection_iter() {
    let mut collection = NodePurposeCollection::new();
    collection.add(NodePurpose::new("a", "A"));
    collection.add(NodePurpose::new("b", "B"));
    collection.add(NodePurpose::new("c", "C"));

    let mut count = 0;
    for (name, purpose) in collection.iter() {
        assert!(!name.is_empty());
        assert!(!purpose.purpose.is_empty());
        count += 1;
    }
    assert_eq!(count, 3);
}

#[test]
fn test_discovered_modules_generated() {
    // Verify build.rs generated the discovered modules correctly
    use super::DISCOVERED_MODULES;

    // Should have discovered a non-trivial number of modules
    assert!(
        DISCOVERED_MODULES.len() >= 100,
        "Expected at least 100 modules, found {}",
        DISCOVERED_MODULES.len()
    );

    // Find distillation module - should have CLI markers
    let distillation = DISCOVERED_MODULES.iter().find(|m| m.name == "distillation");
    assert!(distillation.is_some(), "Should find distillation module");
    let d = distillation.unwrap();
    assert_eq!(d.cli_command, Some("dashflow train distill"));
    assert_eq!(d.cli_status, Some("wired"));
    assert_eq!(d.path, "optimize::distillation");

    // Find cost_monitoring - should be deprecated
    let cost_monitoring = DISCOVERED_MODULES
        .iter()
        .find(|m| m.name == "cost_monitoring");
    assert!(
        cost_monitoring.is_some(),
        "Should find cost_monitoring module"
    );
    assert_eq!(cost_monitoring.unwrap().status, "deprecated");
}
