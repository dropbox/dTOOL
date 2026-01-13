// Allow clippy warnings for capability introspection
// - needless_pass_by_value: Capability fields passed by value for struct construction
#![allow(clippy::needless_pass_by_value)]

//! Capability Introspection
//!
//! This module provides types for AI agents to understand their available capabilities,
//! including tools, models, and storage backends.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Capability Introspection
// ============================================================================

/// Capability manifest - what the AI agent can do
///
/// This struct enumerates all the capabilities available to an AI agent,
/// including tools, LLM models, and storage backends.
///
/// # Example
///
/// ```rust,ignore
/// let caps = graph.capabilities();
///
/// // AI can ask: "Can I write files?"
/// let can_write = caps.tools.iter()
///     .any(|t| t.name == "write_file");
///
/// // AI can ask: "Which LLMs can I use?"
/// let models: Vec<_> = caps.models.iter()
///     .map(|m| &m.name)
///     .collect();
///
/// // AI can ask: "Do I have persistent storage?"
/// let has_db = caps.storage.iter()
///     .any(|s| s.storage_type == StorageType::Database);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityManifest {
    /// Tools available to the agent
    pub tools: Vec<ToolManifest>,
    /// LLM models available for use
    pub models: Vec<ModelCapability>,
    /// Storage backends available
    pub storage: Vec<StorageBackend>,
    /// Custom capabilities (extensible)
    pub custom: HashMap<String, serde_json::Value>,
}

impl CapabilityManifest {
    /// Create a new empty capability manifest
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for capability manifests
    #[must_use]
    pub fn builder() -> CapabilityManifestBuilder {
        CapabilityManifestBuilder::new()
    }

    /// Convert to JSON string
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convert to compact JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse from JSON string
    ///
    /// # Errors
    ///
    /// Returns error if deserialization fails
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Check if a specific tool is available
    #[must_use]
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name == name)
    }

    /// Get a tool by name
    #[must_use]
    pub fn get_tool(&self, name: &str) -> Option<&ToolManifest> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// Get tools by category
    #[must_use]
    pub fn tools_in_category(&self, category: &str) -> Vec<&ToolManifest> {
        self.tools
            .iter()
            .filter(|t| t.category.as_deref() == Some(category))
            .collect()
    }

    /// Check if a specific model is available
    #[must_use]
    pub fn has_model(&self, name: &str) -> bool {
        self.models.iter().any(|m| m.name == name)
    }

    /// Get a model by name
    #[must_use]
    pub fn get_model(&self, name: &str) -> Option<&ModelCapability> {
        self.models.iter().find(|m| m.name == name)
    }

    /// Get models by provider
    #[must_use]
    pub fn models_by_provider(&self, provider: &str) -> Vec<&ModelCapability> {
        self.models
            .iter()
            .filter(|m| m.provider.as_deref() == Some(provider))
            .collect()
    }

    /// Check if a specific storage type is available
    #[must_use]
    pub fn has_storage_type(&self, storage_type: StorageType) -> bool {
        self.storage.iter().any(|s| s.storage_type == storage_type)
    }

    /// Get total tool count
    #[must_use]
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Get total model count
    #[must_use]
    pub fn model_count(&self) -> usize {
        self.models.len()
    }

    /// Get all tool names
    #[must_use]
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name.as_str()).collect()
    }

    /// Get all model names
    #[must_use]
    pub fn model_names(&self) -> Vec<&str> {
        self.models.iter().map(|m| m.name.as_str()).collect()
    }

    /// Check if the agent has any tools
    #[must_use]
    pub fn has_tools(&self) -> bool {
        !self.tools.is_empty()
    }

    /// Check if the agent has any models
    #[must_use]
    pub fn has_models(&self) -> bool {
        !self.models.is_empty()
    }

    /// Check if the agent has any storage
    #[must_use]
    pub fn has_storage(&self) -> bool {
        !self.storage.is_empty()
    }
}

/// Builder for capability manifests
#[derive(Debug, Default)]
pub struct CapabilityManifestBuilder {
    tools: Vec<ToolManifest>,
    models: Vec<ModelCapability>,
    storage: Vec<StorageBackend>,
    custom: HashMap<String, serde_json::Value>,
}

impl CapabilityManifestBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tool
    #[must_use]
    pub fn add_tool(mut self, tool: ToolManifest) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add multiple tools
    #[must_use]
    pub fn tools(mut self, tools: Vec<ToolManifest>) -> Self {
        self.tools = tools;
        self
    }

    /// Add a model
    #[must_use]
    pub fn add_model(mut self, model: ModelCapability) -> Self {
        self.models.push(model);
        self
    }

    /// Add multiple models
    #[must_use]
    pub fn models(mut self, models: Vec<ModelCapability>) -> Self {
        self.models = models;
        self
    }

    /// Add a storage backend
    #[must_use]
    pub fn add_storage(mut self, storage: StorageBackend) -> Self {
        self.storage.push(storage);
        self
    }

    /// Add multiple storage backends
    #[must_use]
    pub fn storage(mut self, storage: Vec<StorageBackend>) -> Self {
        self.storage = storage;
        self
    }

    /// Add custom capability
    #[must_use]
    pub fn custom(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.custom.insert(key.into(), value);
        self
    }

    /// Build the capability manifest
    #[must_use]
    pub fn build(self) -> CapabilityManifest {
        CapabilityManifest {
            tools: self.tools,
            models: self.models,
            storage: self.storage,
            custom: self.custom,
        }
    }
}

/// Tool manifest - describes a tool available to the agent
///
/// # Example
///
/// ```rust,ignore
/// let tool = ToolManifest::new("search", "Search the web for information")
///     .with_category("web")
///     .with_parameter("query", "string", "The search query", true)
///     .with_parameter("limit", "number", "Max results", false);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolManifest {
    /// Tool name (identifier)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Category for grouping (e.g., "web", "filesystem", "code")
    pub category: Option<String>,
    /// Parameters the tool accepts
    pub parameters: Vec<ToolParameter>,
    /// Return type description
    pub returns: Option<String>,
    /// Whether the tool has side effects
    pub has_side_effects: bool,
    /// Whether the tool requires confirmation before execution
    pub requires_confirmation: bool,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ToolManifest {
    /// Create a new tool manifest
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            category: None,
            parameters: Vec::new(),
            returns: None,
            has_side_effects: false,
            requires_confirmation: false,
            metadata: HashMap::new(),
        }
    }

    /// Set category
    #[must_use]
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Add a parameter
    #[must_use]
    pub fn with_parameter(
        mut self,
        name: impl Into<String>,
        param_type: impl Into<String>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        self.parameters.push(ToolParameter {
            name: name.into(),
            param_type: param_type.into(),
            description: description.into(),
            required,
            default_value: None,
        });
        self
    }

    /// Add a parameter with default value
    #[must_use]
    pub fn with_parameter_default(
        mut self,
        name: impl Into<String>,
        param_type: impl Into<String>,
        description: impl Into<String>,
        default: serde_json::Value,
    ) -> Self {
        self.parameters.push(ToolParameter {
            name: name.into(),
            param_type: param_type.into(),
            description: description.into(),
            required: false,
            default_value: Some(default),
        });
        self
    }

    /// Set return type description
    #[must_use]
    pub fn with_returns(mut self, returns: impl Into<String>) -> Self {
        self.returns = Some(returns.into());
        self
    }

    /// Mark as having side effects
    #[must_use]
    pub fn with_side_effects(mut self) -> Self {
        self.has_side_effects = true;
        self
    }

    /// Mark as requiring confirmation
    #[must_use]
    pub fn with_confirmation(mut self) -> Self {
        self.requires_confirmation = true;
        self
    }

    /// Add custom metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Tool parameter description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    /// Parameter name
    pub name: String,
    /// Parameter type (string, number, boolean, object, array, etc.)
    pub param_type: String,
    /// Description of the parameter
    pub description: String,
    /// Whether the parameter is required
    pub required: bool,
    /// Default value if not provided
    pub default_value: Option<serde_json::Value>,
}

/// Model capability - describes an LLM model available to the agent
///
/// # Example
///
/// ```rust,ignore
/// let model = ModelCapability::new("gpt-4", "OpenAI GPT-4")
///     .with_provider("openai")
///     .with_context_window(128000)
///     .with_capability(ModelFeature::Chat)
///     .with_capability(ModelFeature::FunctionCalling);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapability {
    /// Model identifier
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Model provider (e.g., "openai", "anthropic", "google")
    pub provider: Option<String>,
    /// Maximum context window size (in tokens)
    pub context_window: Option<u32>,
    /// Maximum output tokens
    pub max_output_tokens: Option<u32>,
    /// Supported features
    pub features: Vec<ModelFeature>,
    /// Cost per 1K input tokens (USD)
    pub cost_per_1k_input: Option<f64>,
    /// Cost per 1K output tokens (USD)
    pub cost_per_1k_output: Option<f64>,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ModelCapability {
    /// Create a new model capability
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            provider: None,
            context_window: None,
            max_output_tokens: None,
            features: Vec::new(),
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            metadata: HashMap::new(),
        }
    }

    /// Set provider
    #[must_use]
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Set context window size
    #[must_use]
    pub fn with_context_window(mut self, tokens: u32) -> Self {
        self.context_window = Some(tokens);
        self
    }

    /// Set max output tokens
    #[must_use]
    pub fn with_max_output(mut self, tokens: u32) -> Self {
        self.max_output_tokens = Some(tokens);
        self
    }

    /// Add a feature/capability
    #[must_use]
    pub fn with_feature(mut self, feature: ModelFeature) -> Self {
        self.features.push(feature);
        self
    }

    /// Set cost per 1K input tokens
    #[must_use]
    pub fn with_input_cost(mut self, cost: f64) -> Self {
        self.cost_per_1k_input = Some(cost);
        self
    }

    /// Set cost per 1K output tokens
    #[must_use]
    pub fn with_output_cost(mut self, cost: f64) -> Self {
        self.cost_per_1k_output = Some(cost);
        self
    }

    /// Add custom metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Check if model supports a specific feature
    #[must_use]
    pub fn supports(&self, feature: &ModelFeature) -> bool {
        self.features.contains(feature)
    }
}

/// Model features/capabilities
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFeature {
    /// Basic chat completion
    Chat,
    /// Function/tool calling
    FunctionCalling,
    /// JSON mode output
    JsonMode,
    /// Vision/image understanding
    Vision,
    /// Code generation/interpretation
    CodeGeneration,
    /// Embedding generation
    Embeddings,
    /// Streaming responses
    Streaming,
    /// Fine-tuning support
    FineTuning,
    /// Custom feature
    Custom(String),
}

/// Storage backend - describes a storage option available to the agent
///
/// # Example
///
/// ```rust,ignore
/// let storage = StorageBackend::new("checkpoint_db", StorageType::Database)
///     .with_description("SQLite checkpoint storage")
///     .with_feature(StorageFeature::Persistent)
///     .with_feature(StorageFeature::ACID);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageBackend {
    /// Storage identifier
    pub name: String,
    /// Storage type
    pub storage_type: StorageType,
    /// Human-readable description
    pub description: Option<String>,
    /// Storage features
    pub features: Vec<StorageFeature>,
    /// Maximum capacity (if applicable)
    pub max_capacity_bytes: Option<u64>,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl StorageBackend {
    /// Create a new storage backend
    #[must_use]
    pub fn new(name: impl Into<String>, storage_type: StorageType) -> Self {
        Self {
            name: name.into(),
            storage_type,
            description: None,
            features: Vec::new(),
            max_capacity_bytes: None,
            metadata: HashMap::new(),
        }
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a feature
    #[must_use]
    pub fn with_feature(mut self, feature: StorageFeature) -> Self {
        self.features.push(feature);
        self
    }

    /// Set max capacity
    #[must_use]
    pub fn with_max_capacity(mut self, bytes: u64) -> Self {
        self.max_capacity_bytes = Some(bytes);
        self
    }

    /// Add custom metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Check if storage supports a specific feature
    #[must_use]
    pub fn supports(&self, feature: &StorageFeature) -> bool {
        self.features.contains(feature)
    }
}

/// Storage type classification
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageType {
    /// In-memory storage (not persistent)
    #[default]
    Memory,
    /// File system storage
    FileSystem,
    /// Database (SQL or NoSQL)
    Database,
    /// Object storage (S3, GCS, etc.)
    ObjectStorage,
    /// Cache (Redis, Memcached, etc.)
    Cache,
    /// Vector database
    VectorDatabase,
    /// Custom storage type
    Custom(String),
}

/// Storage features
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageFeature {
    /// Data persists across restarts
    Persistent,
    /// Supports ACID transactions
    Acid,
    /// Supports concurrent access
    Concurrent,
    /// Supports encryption at rest
    Encrypted,
    /// Supports replication
    Replicated,
    /// Supports full-text search
    FullTextSearch,
    /// Supports vector similarity search
    VectorSearch,
    /// Custom feature
    Custom(String),
}

// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========================================================================
    // CapabilityManifest Tests
    // ========================================================================

    #[test]
    fn test_capability_manifest_new() {
        let manifest = CapabilityManifest::new();
        assert!(manifest.tools.is_empty());
        assert!(manifest.models.is_empty());
        assert!(manifest.storage.is_empty());
        assert!(manifest.custom.is_empty());
    }

    #[test]
    fn test_capability_manifest_default() {
        let manifest = CapabilityManifest::default();
        assert!(manifest.tools.is_empty());
        assert!(manifest.models.is_empty());
        assert!(manifest.storage.is_empty());
    }

    #[test]
    fn test_capability_manifest_builder_basic() {
        let manifest = CapabilityManifest::builder().build();
        assert!(manifest.tools.is_empty());
        assert!(manifest.models.is_empty());
    }

    #[test]
    fn test_capability_manifest_builder_add_tool() {
        let tool = ToolManifest::new("search", "Search the web");
        let manifest = CapabilityManifest::builder().add_tool(tool).build();

        assert_eq!(manifest.tools.len(), 1);
        assert_eq!(manifest.tools[0].name, "search");
    }

    #[test]
    fn test_capability_manifest_builder_tools_batch() {
        let tools = vec![
            ToolManifest::new("search", "Search the web"),
            ToolManifest::new("write", "Write to file"),
        ];
        let manifest = CapabilityManifest::builder().tools(tools).build();

        assert_eq!(manifest.tools.len(), 2);
    }

    #[test]
    fn test_capability_manifest_builder_add_model() {
        let model = ModelCapability::new("gpt-4", "GPT-4 model");
        let manifest = CapabilityManifest::builder().add_model(model).build();

        assert_eq!(manifest.models.len(), 1);
        assert_eq!(manifest.models[0].name, "gpt-4");
    }

    #[test]
    fn test_capability_manifest_builder_models_batch() {
        let models = vec![
            ModelCapability::new("gpt-4", "GPT-4"),
            ModelCapability::new("claude", "Claude"),
        ];
        let manifest = CapabilityManifest::builder().models(models).build();

        assert_eq!(manifest.models.len(), 2);
    }

    #[test]
    fn test_capability_manifest_builder_add_storage() {
        let storage = StorageBackend::new("db", StorageType::Database);
        let manifest = CapabilityManifest::builder().add_storage(storage).build();

        assert_eq!(manifest.storage.len(), 1);
        assert_eq!(manifest.storage[0].name, "db");
    }

    #[test]
    fn test_capability_manifest_builder_storage_batch() {
        let storage = vec![
            StorageBackend::new("db", StorageType::Database),
            StorageBackend::new("cache", StorageType::Cache),
        ];
        let manifest = CapabilityManifest::builder().storage(storage).build();

        assert_eq!(manifest.storage.len(), 2);
    }

    #[test]
    fn test_capability_manifest_builder_custom() {
        let manifest = CapabilityManifest::builder()
            .custom("version", json!("1.0"))
            .custom("features", json!(["a", "b"]))
            .build();

        assert_eq!(manifest.custom.len(), 2);
        assert_eq!(manifest.custom.get("version"), Some(&json!("1.0")));
    }

    #[test]
    fn test_capability_manifest_to_json() {
        let manifest = CapabilityManifest::new();
        let json = manifest.to_json().unwrap();
        assert!(json.contains("tools"));
        assert!(json.contains("models"));
        assert!(json.contains("storage"));
    }

    #[test]
    fn test_capability_manifest_to_json_compact() {
        let manifest = CapabilityManifest::new();
        let json = manifest.to_json_compact().unwrap();
        assert!(!json.contains('\n'));
    }

    #[test]
    fn test_capability_manifest_from_json() {
        let json = r#"{"tools":[],"models":[],"storage":[],"custom":{}}"#;
        let manifest = CapabilityManifest::from_json(json).unwrap();
        assert!(manifest.tools.is_empty());
    }

    #[test]
    fn test_capability_manifest_json_roundtrip() {
        let tool = ToolManifest::new("search", "Search");
        let model = ModelCapability::new("gpt-4", "GPT-4");
        let storage = StorageBackend::new("db", StorageType::Database);

        let manifest = CapabilityManifest::builder()
            .add_tool(tool)
            .add_model(model)
            .add_storage(storage)
            .custom("key", json!("value"))
            .build();

        let json = manifest.to_json().unwrap();
        let restored = CapabilityManifest::from_json(&json).unwrap();

        assert_eq!(manifest.tools.len(), restored.tools.len());
        assert_eq!(manifest.models.len(), restored.models.len());
        assert_eq!(manifest.storage.len(), restored.storage.len());
        assert_eq!(manifest.custom.len(), restored.custom.len());
    }

    #[test]
    fn test_capability_manifest_has_tool() {
        let manifest = CapabilityManifest::builder()
            .add_tool(ToolManifest::new("search", "Search"))
            .build();

        assert!(manifest.has_tool("search"));
        assert!(!manifest.has_tool("write"));
    }

    #[test]
    fn test_capability_manifest_get_tool() {
        let manifest = CapabilityManifest::builder()
            .add_tool(ToolManifest::new("search", "Search the web"))
            .build();

        let tool = manifest.get_tool("search");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().description, "Search the web");

        assert!(manifest.get_tool("nonexistent").is_none());
    }

    #[test]
    fn test_capability_manifest_tools_in_category() {
        let manifest = CapabilityManifest::builder()
            .add_tool(ToolManifest::new("search", "Search").with_category("web"))
            .add_tool(ToolManifest::new("fetch", "Fetch").with_category("web"))
            .add_tool(ToolManifest::new("write", "Write").with_category("fs"))
            .build();

        let web_tools = manifest.tools_in_category("web");
        assert_eq!(web_tools.len(), 2);

        let fs_tools = manifest.tools_in_category("fs");
        assert_eq!(fs_tools.len(), 1);

        let empty = manifest.tools_in_category("nonexistent");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_capability_manifest_has_model() {
        let manifest = CapabilityManifest::builder()
            .add_model(ModelCapability::new("gpt-4", "GPT-4"))
            .build();

        assert!(manifest.has_model("gpt-4"));
        assert!(!manifest.has_model("claude"));
    }

    #[test]
    fn test_capability_manifest_get_model() {
        let manifest = CapabilityManifest::builder()
            .add_model(ModelCapability::new("gpt-4", "GPT-4 model"))
            .build();

        let model = manifest.get_model("gpt-4");
        assert!(model.is_some());
        assert_eq!(model.unwrap().description, "GPT-4 model");

        assert!(manifest.get_model("nonexistent").is_none());
    }

    #[test]
    fn test_capability_manifest_models_by_provider() {
        let manifest = CapabilityManifest::builder()
            .add_model(ModelCapability::new("gpt-4", "GPT-4").with_provider("openai"))
            .add_model(ModelCapability::new("gpt-3.5", "GPT-3.5").with_provider("openai"))
            .add_model(ModelCapability::new("claude", "Claude").with_provider("anthropic"))
            .build();

        let openai_models = manifest.models_by_provider("openai");
        assert_eq!(openai_models.len(), 2);

        let anthropic_models = manifest.models_by_provider("anthropic");
        assert_eq!(anthropic_models.len(), 1);

        let empty = manifest.models_by_provider("google");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_capability_manifest_has_storage_type() {
        let manifest = CapabilityManifest::builder()
            .add_storage(StorageBackend::new("db", StorageType::Database))
            .add_storage(StorageBackend::new("cache", StorageType::Cache))
            .build();

        assert!(manifest.has_storage_type(StorageType::Database));
        assert!(manifest.has_storage_type(StorageType::Cache));
        assert!(!manifest.has_storage_type(StorageType::FileSystem));
    }

    #[test]
    fn test_capability_manifest_counts() {
        let manifest = CapabilityManifest::builder()
            .add_tool(ToolManifest::new("t1", "Tool 1"))
            .add_tool(ToolManifest::new("t2", "Tool 2"))
            .add_model(ModelCapability::new("m1", "Model 1"))
            .build();

        assert_eq!(manifest.tool_count(), 2);
        assert_eq!(manifest.model_count(), 1);
    }

    #[test]
    fn test_capability_manifest_names() {
        let manifest = CapabilityManifest::builder()
            .add_tool(ToolManifest::new("search", "Search"))
            .add_tool(ToolManifest::new("write", "Write"))
            .add_model(ModelCapability::new("gpt-4", "GPT-4"))
            .build();

        let tool_names = manifest.tool_names();
        assert_eq!(tool_names.len(), 2);
        assert!(tool_names.contains(&"search"));
        assert!(tool_names.contains(&"write"));

        let model_names = manifest.model_names();
        assert_eq!(model_names.len(), 1);
        assert!(model_names.contains(&"gpt-4"));
    }

    #[test]
    fn test_capability_manifest_has_checks() {
        let empty = CapabilityManifest::new();
        assert!(!empty.has_tools());
        assert!(!empty.has_models());
        assert!(!empty.has_storage());

        let manifest = CapabilityManifest::builder()
            .add_tool(ToolManifest::new("t", "Tool"))
            .add_model(ModelCapability::new("m", "Model"))
            .add_storage(StorageBackend::new("s", StorageType::Memory))
            .build();

        assert!(manifest.has_tools());
        assert!(manifest.has_models());
        assert!(manifest.has_storage());
    }

    // ========================================================================
    // ToolManifest Tests
    // ========================================================================

    #[test]
    fn test_tool_manifest_new() {
        let tool = ToolManifest::new("search", "Search the web");
        assert_eq!(tool.name, "search");
        assert_eq!(tool.description, "Search the web");
        assert!(tool.category.is_none());
        assert!(tool.parameters.is_empty());
        assert!(tool.returns.is_none());
        assert!(!tool.has_side_effects);
        assert!(!tool.requires_confirmation);
        assert!(tool.metadata.is_empty());
    }

    #[test]
    fn test_tool_manifest_with_category() {
        let tool = ToolManifest::new("search", "Search").with_category("web");
        assert_eq!(tool.category, Some("web".to_string()));
    }

    #[test]
    fn test_tool_manifest_with_parameter() {
        let tool = ToolManifest::new("search", "Search")
            .with_parameter("query", "string", "Search query", true)
            .with_parameter("limit", "number", "Max results", false);

        assert_eq!(tool.parameters.len(), 2);

        let query_param = &tool.parameters[0];
        assert_eq!(query_param.name, "query");
        assert_eq!(query_param.param_type, "string");
        assert_eq!(query_param.description, "Search query");
        assert!(query_param.required);
        assert!(query_param.default_value.is_none());

        let limit_param = &tool.parameters[1];
        assert_eq!(limit_param.name, "limit");
        assert!(!limit_param.required);
    }

    #[test]
    fn test_tool_manifest_with_parameter_default() {
        let tool = ToolManifest::new("search", "Search").with_parameter_default(
            "limit",
            "number",
            "Max results",
            json!(10),
        );

        assert_eq!(tool.parameters.len(), 1);
        let param = &tool.parameters[0];
        assert!(!param.required);
        assert_eq!(param.default_value, Some(json!(10)));
    }

    #[test]
    fn test_tool_manifest_with_returns() {
        let tool = ToolManifest::new("search", "Search").with_returns("Array of search results");
        assert_eq!(tool.returns, Some("Array of search results".to_string()));
    }

    #[test]
    fn test_tool_manifest_with_side_effects() {
        let tool = ToolManifest::new("write", "Write file").with_side_effects();
        assert!(tool.has_side_effects);
    }

    #[test]
    fn test_tool_manifest_with_confirmation() {
        let tool = ToolManifest::new("delete", "Delete file").with_confirmation();
        assert!(tool.requires_confirmation);
    }

    #[test]
    fn test_tool_manifest_with_metadata() {
        let tool = ToolManifest::new("search", "Search")
            .with_metadata("version", json!("1.0"))
            .with_metadata("author", json!("Alice"));

        assert_eq!(tool.metadata.len(), 2);
        assert_eq!(tool.metadata.get("version"), Some(&json!("1.0")));
    }

    #[test]
    fn test_tool_manifest_serialization() {
        let tool = ToolManifest::new("search", "Search")
            .with_category("web")
            .with_parameter("query", "string", "Query", true);

        let json = serde_json::to_string(&tool).unwrap();
        let restored: ToolManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(tool.name, restored.name);
        assert_eq!(tool.category, restored.category);
        assert_eq!(tool.parameters.len(), restored.parameters.len());
    }

    // ========================================================================
    // ModelCapability Tests
    // ========================================================================

    #[test]
    fn test_model_capability_new() {
        let model = ModelCapability::new("gpt-4", "GPT-4 model");
        assert_eq!(model.name, "gpt-4");
        assert_eq!(model.description, "GPT-4 model");
        assert!(model.provider.is_none());
        assert!(model.context_window.is_none());
        assert!(model.max_output_tokens.is_none());
        assert!(model.features.is_empty());
        assert!(model.cost_per_1k_input.is_none());
        assert!(model.cost_per_1k_output.is_none());
    }

    #[test]
    fn test_model_capability_with_provider() {
        let model = ModelCapability::new("gpt-4", "GPT-4").with_provider("openai");
        assert_eq!(model.provider, Some("openai".to_string()));
    }

    #[test]
    fn test_model_capability_with_context_window() {
        let model = ModelCapability::new("gpt-4", "GPT-4").with_context_window(128000);
        assert_eq!(model.context_window, Some(128000));
    }

    #[test]
    fn test_model_capability_with_max_output() {
        let model = ModelCapability::new("gpt-4", "GPT-4").with_max_output(4096);
        assert_eq!(model.max_output_tokens, Some(4096));
    }

    #[test]
    fn test_model_capability_with_feature() {
        let model = ModelCapability::new("gpt-4", "GPT-4")
            .with_feature(ModelFeature::Chat)
            .with_feature(ModelFeature::FunctionCalling);

        assert_eq!(model.features.len(), 2);
        assert!(model.features.contains(&ModelFeature::Chat));
        assert!(model.features.contains(&ModelFeature::FunctionCalling));
    }

    #[test]
    fn test_model_capability_with_costs() {
        let model = ModelCapability::new("gpt-4", "GPT-4")
            .with_input_cost(0.03)
            .with_output_cost(0.06);

        assert_eq!(model.cost_per_1k_input, Some(0.03));
        assert_eq!(model.cost_per_1k_output, Some(0.06));
    }

    #[test]
    fn test_model_capability_with_metadata() {
        let model =
            ModelCapability::new("gpt-4", "GPT-4").with_metadata("version", json!("2024-01"));

        assert_eq!(model.metadata.get("version"), Some(&json!("2024-01")));
    }

    #[test]
    fn test_model_capability_supports() {
        let model = ModelCapability::new("gpt-4", "GPT-4")
            .with_feature(ModelFeature::Chat)
            .with_feature(ModelFeature::Vision);

        assert!(model.supports(&ModelFeature::Chat));
        assert!(model.supports(&ModelFeature::Vision));
        assert!(!model.supports(&ModelFeature::Embeddings));
    }

    #[test]
    fn test_model_capability_serialization() {
        let model = ModelCapability::new("gpt-4", "GPT-4")
            .with_provider("openai")
            .with_feature(ModelFeature::Chat);

        let json = serde_json::to_string(&model).unwrap();
        let restored: ModelCapability = serde_json::from_str(&json).unwrap();

        assert_eq!(model.name, restored.name);
        assert_eq!(model.provider, restored.provider);
    }

    // ========================================================================
    // ModelFeature Tests
    // ========================================================================

    #[test]
    fn test_model_feature_variants() {
        let features = vec![
            ModelFeature::Chat,
            ModelFeature::FunctionCalling,
            ModelFeature::JsonMode,
            ModelFeature::Vision,
            ModelFeature::CodeGeneration,
            ModelFeature::Embeddings,
            ModelFeature::Streaming,
            ModelFeature::FineTuning,
            ModelFeature::Custom("special".to_string()),
        ];

        assert_eq!(features.len(), 9);
    }

    #[test]
    fn test_model_feature_equality() {
        assert_eq!(ModelFeature::Chat, ModelFeature::Chat);
        assert_ne!(ModelFeature::Chat, ModelFeature::Vision);
        assert_eq!(
            ModelFeature::Custom("a".to_string()),
            ModelFeature::Custom("a".to_string())
        );
        assert_ne!(
            ModelFeature::Custom("a".to_string()),
            ModelFeature::Custom("b".to_string())
        );
    }

    #[test]
    fn test_model_feature_serialization() {
        let feature = ModelFeature::FunctionCalling;
        let json = serde_json::to_string(&feature).unwrap();
        assert_eq!(json, "\"function_calling\"");

        let custom = ModelFeature::Custom("special".to_string());
        let json = serde_json::to_string(&custom).unwrap();
        assert!(json.contains("special"));
    }

    // ========================================================================
    // StorageBackend Tests
    // ========================================================================

    #[test]
    fn test_storage_backend_new() {
        let storage = StorageBackend::new("db", StorageType::Database);
        assert_eq!(storage.name, "db");
        assert_eq!(storage.storage_type, StorageType::Database);
        assert!(storage.description.is_none());
        assert!(storage.features.is_empty());
        assert!(storage.max_capacity_bytes.is_none());
    }

    #[test]
    fn test_storage_backend_with_description() {
        let storage = StorageBackend::new("db", StorageType::Database)
            .with_description("PostgreSQL database");
        assert_eq!(storage.description, Some("PostgreSQL database".to_string()));
    }

    #[test]
    fn test_storage_backend_with_feature() {
        let storage = StorageBackend::new("db", StorageType::Database)
            .with_feature(StorageFeature::Persistent)
            .with_feature(StorageFeature::Acid);

        assert_eq!(storage.features.len(), 2);
        assert!(storage.features.contains(&StorageFeature::Persistent));
        assert!(storage.features.contains(&StorageFeature::Acid));
    }

    #[test]
    fn test_storage_backend_with_max_capacity() {
        let storage =
            StorageBackend::new("cache", StorageType::Cache).with_max_capacity(1024 * 1024 * 100); // 100MB

        assert_eq!(storage.max_capacity_bytes, Some(104857600));
    }

    #[test]
    fn test_storage_backend_with_metadata() {
        let storage = StorageBackend::new("db", StorageType::Database)
            .with_metadata("host", json!("localhost"));

        assert_eq!(storage.metadata.get("host"), Some(&json!("localhost")));
    }

    #[test]
    fn test_storage_backend_supports() {
        let storage = StorageBackend::new("db", StorageType::Database)
            .with_feature(StorageFeature::Persistent)
            .with_feature(StorageFeature::Acid);

        assert!(storage.supports(&StorageFeature::Persistent));
        assert!(storage.supports(&StorageFeature::Acid));
        assert!(!storage.supports(&StorageFeature::VectorSearch));
    }

    #[test]
    fn test_storage_backend_serialization() {
        let storage = StorageBackend::new("db", StorageType::Database)
            .with_feature(StorageFeature::Persistent);

        let json = serde_json::to_string(&storage).unwrap();
        let restored: StorageBackend = serde_json::from_str(&json).unwrap();

        assert_eq!(storage.name, restored.name);
        assert_eq!(storage.storage_type, restored.storage_type);
    }

    // ========================================================================
    // StorageType Tests
    // ========================================================================

    #[test]
    fn test_storage_type_default() {
        let default = StorageType::default();
        assert_eq!(default, StorageType::Memory);
    }

    #[test]
    fn test_storage_type_variants() {
        let types = vec![
            StorageType::Memory,
            StorageType::FileSystem,
            StorageType::Database,
            StorageType::ObjectStorage,
            StorageType::Cache,
            StorageType::VectorDatabase,
            StorageType::Custom("custom".to_string()),
        ];

        assert_eq!(types.len(), 7);
    }

    #[test]
    fn test_storage_type_equality() {
        assert_eq!(StorageType::Database, StorageType::Database);
        assert_ne!(StorageType::Database, StorageType::Cache);
        assert_eq!(
            StorageType::Custom("a".to_string()),
            StorageType::Custom("a".to_string())
        );
    }

    #[test]
    fn test_storage_type_serialization() {
        let st = StorageType::Database;
        let json = serde_json::to_string(&st).unwrap();
        assert_eq!(json, "\"database\"");

        let custom = StorageType::Custom("s3".to_string());
        let json = serde_json::to_string(&custom).unwrap();
        assert!(json.contains("s3"));
    }

    // ========================================================================
    // StorageFeature Tests
    // ========================================================================

    #[test]
    fn test_storage_feature_variants() {
        let features = vec![
            StorageFeature::Persistent,
            StorageFeature::Acid,
            StorageFeature::Concurrent,
            StorageFeature::Encrypted,
            StorageFeature::Replicated,
            StorageFeature::FullTextSearch,
            StorageFeature::VectorSearch,
            StorageFeature::Custom("custom".to_string()),
        ];

        assert_eq!(features.len(), 8);
    }

    #[test]
    fn test_storage_feature_equality() {
        assert_eq!(StorageFeature::Persistent, StorageFeature::Persistent);
        assert_ne!(StorageFeature::Persistent, StorageFeature::Encrypted);
    }

    #[test]
    fn test_storage_feature_serialization() {
        let feature = StorageFeature::FullTextSearch;
        let json = serde_json::to_string(&feature).unwrap();
        assert_eq!(json, "\"full_text_search\"");
    }

    // ========================================================================
    // Complex Scenario Tests
    // ========================================================================

    #[test]
    fn test_full_capability_manifest() {
        let manifest = CapabilityManifest::builder()
            .add_tool(
                ToolManifest::new("search", "Search the web")
                    .with_category("web")
                    .with_parameter("query", "string", "Search query", true)
                    .with_parameter_default("limit", "number", "Max results", json!(10))
                    .with_returns("Array of results"),
            )
            .add_tool(
                ToolManifest::new("write_file", "Write to file")
                    .with_category("filesystem")
                    .with_parameter("path", "string", "File path", true)
                    .with_parameter("content", "string", "Content", true)
                    .with_side_effects()
                    .with_confirmation(),
            )
            .add_model(
                ModelCapability::new("gpt-4", "OpenAI GPT-4")
                    .with_provider("openai")
                    .with_context_window(128000)
                    .with_max_output(4096)
                    .with_feature(ModelFeature::Chat)
                    .with_feature(ModelFeature::FunctionCalling)
                    .with_feature(ModelFeature::Vision)
                    .with_input_cost(0.03)
                    .with_output_cost(0.06),
            )
            .add_storage(
                StorageBackend::new("postgresql", StorageType::Database)
                    .with_description("PostgreSQL checkpoint storage")
                    .with_feature(StorageFeature::Persistent)
                    .with_feature(StorageFeature::Acid)
                    .with_feature(StorageFeature::Concurrent),
            )
            .add_storage(
                StorageBackend::new("redis", StorageType::Cache)
                    .with_description("Redis cache")
                    .with_feature(StorageFeature::Concurrent)
                    .with_max_capacity(1024 * 1024 * 1024),
            )
            .custom("version", json!("1.0.0"))
            .build();

        // Verify tools
        assert_eq!(manifest.tool_count(), 2);
        assert!(manifest.has_tool("search"));
        assert!(manifest.has_tool("write_file"));

        let web_tools = manifest.tools_in_category("web");
        assert_eq!(web_tools.len(), 1);

        let search = manifest.get_tool("search").unwrap();
        assert_eq!(search.parameters.len(), 2);

        // Verify models
        assert_eq!(manifest.model_count(), 1);
        assert!(manifest.has_model("gpt-4"));

        let gpt4 = manifest.get_model("gpt-4").unwrap();
        assert!(gpt4.supports(&ModelFeature::Vision));

        // Verify storage
        assert!(manifest.has_storage_type(StorageType::Database));
        assert!(manifest.has_storage_type(StorageType::Cache));
        assert!(!manifest.has_storage_type(StorageType::Memory));

        // Verify JSON roundtrip
        let json = manifest.to_json().unwrap();
        let restored = CapabilityManifest::from_json(&json).unwrap();
        assert_eq!(manifest.tool_count(), restored.tool_count());
        assert_eq!(manifest.model_count(), restored.model_count());
    }

    #[test]
    fn test_capability_manifest_empty_queries() {
        let manifest = CapabilityManifest::new();

        assert!(!manifest.has_tool("any"));
        assert!(manifest.get_tool("any").is_none());
        assert!(manifest.tools_in_category("any").is_empty());
        assert!(!manifest.has_model("any"));
        assert!(manifest.get_model("any").is_none());
        assert!(manifest.models_by_provider("any").is_empty());
        assert!(!manifest.has_storage_type(StorageType::Database));
    }

    #[test]
    fn test_tool_parameter_complete() {
        let param = ToolParameter {
            name: "count".to_string(),
            param_type: "integer".to_string(),
            description: "Number of items".to_string(),
            required: false,
            default_value: Some(json!(5)),
        };

        assert_eq!(param.name, "count");
        assert!(!param.required);
        assert_eq!(param.default_value, Some(json!(5)));
    }

    #[test]
    fn test_builder_chaining() {
        // Test that all builder methods can be chained
        let manifest = CapabilityManifest::builder()
            .add_tool(ToolManifest::new("t1", "T1"))
            .add_tool(ToolManifest::new("t2", "T2"))
            .add_model(ModelCapability::new("m1", "M1"))
            .add_model(ModelCapability::new("m2", "M2"))
            .add_storage(StorageBackend::new("s1", StorageType::Memory))
            .custom("k1", json!(1))
            .custom("k2", json!(2))
            .build();

        assert_eq!(manifest.tool_count(), 2);
        assert_eq!(manifest.model_count(), 2);
        assert_eq!(manifest.storage.len(), 1);
        assert_eq!(manifest.custom.len(), 2);
    }
}
