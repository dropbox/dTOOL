// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Node Registry - Dynamic Node Discovery and Factory System
//!
//! This module provides dynamic node registration and construction capabilities
//! for AI agents to discover and instantiate nodes at runtime. It enables:
//!
//! - **Node Discovery**: Query "what node types are available?"
//! - **Dynamic Construction**: Create nodes from configurations
//! - **Factory Pattern**: Type-erased factories for heterogeneous node types
//! - **Integration with GraphManifest**: Use `NodeConfig` to reconstruct graphs
//!
//! ## Key Components
//!
//! - [`NodeFactory`]: Trait for creating nodes from configuration
//! - [`NodeRegistry`]: Central registry for node factories
//! - [`FactoryTypeInfo`]: Metadata about registered node types
//!
//! ## Example: Registering and Using Node Factories
//!
//! ```rust,ignore
//! use dashflow::node_registry::{NodeRegistry, NodeFactory, FactoryTypeInfo};
//! use dashflow::Node;
//!
//! // Define a factory for a specific node type
//! struct ChatNodeFactory;
//!
//! impl<S: Send + Sync + 'static> NodeFactory<S> for ChatNodeFactory {
//!     fn create(&self, config: &serde_json::Value) -> Result<Box<dyn Node<S>>, NodeFactoryError> {
//!         let model = config.get("model").and_then(|v| v.as_str()).unwrap_or("gpt-4");
//!         Ok(Box::new(ChatNode::new(model)))
//!     }
//!
//!     fn type_info(&self) -> FactoryTypeInfo {
//!         FactoryTypeInfo::new("llm.chat")
//!             .with_description("LLM chat completion node")
//!             .with_config_schema(json!({
//!                 "type": "object",
//!                 "properties": {
//!                     "model": { "type": "string" },
//!                     "temperature": { "type": "number" }
//!                 }
//!             }))
//!     }
//! }
//!
//! // Register factories
//! let mut registry = NodeRegistry::new();
//! registry.register("llm.chat", Box::new(ChatNodeFactory));
//!
//! // Query available node types
//! let types = registry.list_types();
//! for type_info in types {
//!     println!("Available: {} - {}", type_info.name, type_info.description);
//! }
//!
//! // Create a node from configuration
//! let config = json!({"model": "gpt-4", "temperature": 0.7});
//! let node = registry.create::<MyState>("llm.chat", &config)?;
//! ```
//!
//! ## Integration with GraphManifest
//!
//! The node registry integrates with `NodeConfig` from `introspection` module:
//!
//! ```rust,ignore
//! use dashflow::introspection::{GraphManifest, NodeConfig};
//! use dashflow::node_registry::NodeRegistry;
//!
//! // Load manifest from JSON
//! let manifest: GraphManifest = serde_json::from_str(manifest_json)?;
//!
//! // Reconstruct nodes using registry
//! for (name, config) in &manifest.node_configs {
//!     let node = registry.create::<MyState>(&config.node_type, &config.config)?;
//!     graph.add_node(name, node);
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use thiserror::Error;

use crate::error::Result;
use crate::node::{BoxedNode, Node};
use crate::registry_trait::Registry;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during node factory operations
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum NodeFactoryError {
    /// The requested node type is not registered
    #[error("Unknown node type: '{0}'. Use registry.list_types() to see available types.")]
    UnknownNodeType(String),
    /// Configuration validation failed
    #[error("Invalid configuration for node type '{node_type}': {message}")]
    InvalidConfig {
        /// The node type whose configuration was invalid.
        node_type: String,
        /// Human-readable description of the validation failure.
        message: String,
    },
    /// Factory failed to create the node
    #[error("Failed to create node of type '{node_type}': {message}")]
    CreationFailed {
        /// The node type that failed to be created.
        node_type: String,
        /// Human-readable description of the creation failure.
        message: String,
    },
    /// Type mismatch between factory and requested state type
    #[error(
        "Type mismatch for node '{node_type}': expected state type '{expected}', got '{actual}'"
    )]
    TypeMismatch {
        /// The node type with a type mismatch.
        node_type: String,
        /// Expected state type name.
        expected: String,
        /// Actual state type name that was provided.
        actual: String,
    },
}

// ============================================================================
// Node Type Info - Metadata about registered node types
// ============================================================================

/// Metadata describing a registered node factory type
///
/// This provides discoverability for AI agents to understand what
/// node types are available and how to configure them.
///
/// Note: Named `FactoryTypeInfo` to avoid collision with
/// `platform_introspection::FactoryTypeInfo` which serves a different purpose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryTypeInfo {
    /// Unique identifier for this node type (e.g., "llm.chat", "tool.search")
    pub name: String,
    /// Human-readable description of what this node does
    pub description: String,
    /// Category for grouping (e.g., "llm", "tool", "transform", "io")
    pub category: String,
    /// JSON Schema for the configuration object
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<serde_json::Value>,
    /// Example configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example_config: Option<serde_json::Value>,
    /// Tags for search/discovery
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Version of this node type
    #[serde(default = "default_version")]
    pub version: String,
    /// Whether this node type is deprecated
    #[serde(default)]
    pub deprecated: bool,
    /// Deprecation message if deprecated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecation_message: Option<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

impl FactoryTypeInfo {
    /// Create new node type info
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            category: "general".to_string(),
            config_schema: None,
            example_config: None,
            tags: Vec::new(),
            version: default_version(),
            deprecated: false,
            deprecation_message: None,
        }
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set category
    #[must_use]
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    /// Set JSON schema for configuration
    #[must_use]
    pub fn with_config_schema(mut self, schema: serde_json::Value) -> Self {
        self.config_schema = Some(schema);
        self
    }

    /// Set example configuration
    #[must_use]
    pub fn with_example(mut self, example: serde_json::Value) -> Self {
        self.example_config = Some(example);
        self
    }

    /// Add a tag
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags
    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Set version
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Mark as deprecated
    #[must_use]
    pub fn deprecated(mut self, message: impl Into<String>) -> Self {
        self.deprecated = true;
        self.deprecation_message = Some(message.into());
        self
    }
}

// ============================================================================
// NodeFactory Trait - Type-Safe Node Construction
// ============================================================================

/// Factory trait for creating nodes of a specific type
///
/// Implementations of this trait know how to construct nodes from
/// configuration objects. The factory is parameterized by the state type `S`.
///
/// # Example
///
/// ```rust,ignore
/// struct EchoNodeFactory;
///
/// impl<S: Clone + Send + Sync + 'static> NodeFactory<S> for EchoNodeFactory {
///     fn create(&self, _config: &serde_json::Value) -> std::result::Result<BoxedNode<S>, NodeFactoryError> {
///         Ok(Arc::new(EchoNode))
///     }
///
///     fn type_info(&self) -> FactoryTypeInfo {
///         FactoryTypeInfo::new("echo")
///             .with_description("Echoes the input state unchanged")
///             .with_category("transform")
///     }
/// }
/// ```
pub trait NodeFactory<S>: Send + Sync
where
    S: Send + Sync + 'static,
{
    /// Create a node from the given configuration
    ///
    /// # Arguments
    /// * `config` - JSON configuration for the node
    ///
    /// # Returns
    /// * `Ok(BoxedNode<S>)` - The constructed node
    /// * `Err(NodeFactoryError)` - If construction fails
    fn create(
        &self,
        config: &serde_json::Value,
    ) -> std::result::Result<BoxedNode<S>, NodeFactoryError>;

    /// Get metadata about this node type
    fn type_info(&self) -> FactoryTypeInfo;

    /// Validate configuration without creating a node
    ///
    /// Default implementation always returns Ok. Override to add validation.
    fn validate_config(
        &self,
        _config: &serde_json::Value,
    ) -> std::result::Result<(), NodeFactoryError> {
        Ok(())
    }
}

// ============================================================================
// NodeRegistry - Central Factory Registry
// ============================================================================

/// Central registry for node factories
///
/// The registry stores factories indexed by node type name, enabling
/// dynamic node discovery and construction.
///
/// # Thread Safety
///
/// `NodeRegistry` is designed to be used with `Arc` for shared access:
///
/// ```rust,ignore
/// let registry = Arc::new(NodeRegistry::new());
/// // Share across threads
/// ```
///
/// # Type Parameters
///
/// The registry is parameterized by state type `S`. All factories in a single
/// registry must work with the same state type.
pub struct NodeRegistry<S>
where
    S: Send + Sync + 'static,
{
    /// Registered factories indexed by node type name
    factories: HashMap<String, Arc<dyn NodeFactory<S>>>,
    /// Type info cache for quick lookups
    type_info_cache: HashMap<String, FactoryTypeInfo>,
}

impl<S> Default for NodeRegistry<S>
where
    S: Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> NodeRegistry<S>
where
    S: Send + Sync + 'static,
{
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
            type_info_cache: HashMap::new(),
        }
    }

    /// Register a node factory
    ///
    /// # Arguments
    /// * `node_type` - Unique identifier for this node type
    /// * `factory` - Factory implementation
    ///
    /// # Returns
    /// Previous factory if one was registered with the same name
    pub fn register<F>(
        &mut self,
        node_type: impl Into<String>,
        factory: F,
    ) -> Option<Arc<dyn NodeFactory<S>>>
    where
        F: NodeFactory<S> + 'static,
    {
        let node_type = node_type.into();
        let factory = Arc::new(factory);
        let type_info = factory.type_info();

        self.type_info_cache.insert(node_type.clone(), type_info);
        self.factories.insert(node_type, factory)
    }

    /// Register a factory with an Arc (for shared factories)
    pub fn register_arc(
        &mut self,
        node_type: impl Into<String>,
        factory: Arc<dyn NodeFactory<S>>,
    ) -> Option<Arc<dyn NodeFactory<S>>> {
        let node_type = node_type.into();
        let type_info = factory.type_info();

        self.type_info_cache.insert(node_type.clone(), type_info);
        self.factories.insert(node_type, factory)
    }

    /// Unregister a node factory
    ///
    /// # Returns
    /// The removed factory if it existed
    pub fn unregister(&mut self, node_type: &str) -> Option<Arc<dyn NodeFactory<S>>> {
        self.type_info_cache.remove(node_type);
        self.factories.remove(node_type)
    }

    /// Check if a node type is registered
    #[must_use]
    pub fn contains(&self, node_type: &str) -> bool {
        self.factories.contains_key(node_type)
    }

    /// Get the number of registered factories
    #[must_use]
    pub fn len(&self) -> usize {
        self.factories.len()
    }

    /// Check if the registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }

    /// Create a node from configuration
    ///
    /// # Arguments
    /// * `node_type` - Type of node to create
    /// * `config` - Configuration for the node
    ///
    /// # Returns
    /// The constructed node
    ///
    /// # Errors
    /// * `NodeFactoryError::UnknownNodeType` - If the node type is not registered
    /// * `NodeFactoryError::InvalidConfig` - If configuration validation fails
    /// * `NodeFactoryError::CreationFailed` - If the factory fails to create the node
    pub fn create(
        &self,
        node_type: &str,
        config: &serde_json::Value,
    ) -> std::result::Result<BoxedNode<S>, NodeFactoryError> {
        let factory = self
            .factories
            .get(node_type)
            .ok_or_else(|| NodeFactoryError::UnknownNodeType(node_type.to_string()))?;

        // Check for deprecation warning
        if let Some(info) = self.type_info_cache.get(node_type) {
            if info.deprecated {
                tracing::warn!(
                    node_type = %node_type,
                    message = ?info.deprecation_message,
                    "Creating deprecated node type"
                );
            }
        }

        factory.create(config)
    }

    /// Validate configuration without creating a node
    ///
    /// # Arguments
    /// * `node_type` - Type of node
    /// * `config` - Configuration to validate
    ///
    /// # Errors
    /// Returns error if node type is unknown or config is invalid
    pub fn validate_config(
        &self,
        node_type: &str,
        config: &serde_json::Value,
    ) -> std::result::Result<(), NodeFactoryError> {
        let factory = self
            .factories
            .get(node_type)
            .ok_or_else(|| NodeFactoryError::UnknownNodeType(node_type.to_string()))?;

        factory.validate_config(config)
    }

    /// Get type info for a registered node type
    #[must_use]
    pub fn get_type_info(&self, node_type: &str) -> Option<&FactoryTypeInfo> {
        self.type_info_cache.get(node_type)
    }

    /// List all registered node types
    #[must_use]
    pub fn list_types(&self) -> Vec<&FactoryTypeInfo> {
        self.type_info_cache.values().collect()
    }

    /// List node types by category
    #[must_use]
    pub fn list_by_category(&self, category: &str) -> Vec<&FactoryTypeInfo> {
        self.type_info_cache
            .values()
            .filter(|info| info.category == category)
            .collect()
    }

    /// Find node types by tag
    #[must_use]
    pub fn find_by_tag(&self, tag: &str) -> Vec<&FactoryTypeInfo> {
        self.type_info_cache
            .values()
            .filter(|info| info.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Search node types by name or description
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&FactoryTypeInfo> {
        let query_lower = query.to_lowercase();
        self.type_info_cache
            .values()
            .filter(|info| {
                info.name.to_lowercase().contains(&query_lower)
                    || info.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// Get all unique categories
    #[must_use]
    pub fn categories(&self) -> Vec<String> {
        let mut categories: Vec<String> = self
            .type_info_cache
            .values()
            .map(|info| info.category.clone())
            .collect();
        categories.sort();
        categories.dedup();
        categories
    }

    /// Export registry information as JSON
    ///
    /// This is useful for AI agents to understand available node types.
    ///
    /// # Errors
    /// Returns error if serialization fails
    pub fn to_json(&self) -> std::result::Result<String, serde_json::Error> {
        let types: Vec<&FactoryTypeInfo> = self.list_types();
        serde_json::to_string_pretty(&types)
    }

    /// Get factory reference (for advanced use cases)
    #[must_use]
    pub fn get_factory(&self, node_type: &str) -> Option<&Arc<dyn NodeFactory<S>>> {
        self.factories.get(node_type)
    }
}

impl<S> fmt::Debug for NodeRegistry<S>
where
    S: Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeRegistry")
            .field("factory_count", &self.factories.len())
            .field("types", &self.type_info_cache.keys().collect::<Vec<_>>())
            .finish()
    }
}

// ============================================================================
// Registry Trait Implementation (Phase 2.2 of REFACTORING_PLAN.md)
// ============================================================================

/// Implements the standard Registry trait for NodeRegistry.
///
/// This allows NodeRegistry to be used with generic code that accepts
/// any Registry implementation, improving code reuse and consistency.
impl<S> Registry<Arc<dyn NodeFactory<S>>> for NodeRegistry<S>
where
    S: Send + Sync + 'static,
{
    fn get(&self, key: &str) -> Option<&Arc<dyn NodeFactory<S>>> {
        self.factories.get(key)
    }

    fn contains(&self, key: &str) -> bool {
        self.factories.contains_key(key)
    }

    fn len(&self) -> usize {
        self.factories.len()
    }
}

// ============================================================================
// Built-in Factories
// ============================================================================

/// Factory for creating function nodes
///
/// This factory creates simple pass-through nodes that can be customized
/// via configuration.
pub struct FunctionNodeFactory<S, F>
where
    S: Send + Sync + 'static,
    F: Fn(S) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<S>> + Send>>
        + Send
        + Sync
        + Clone
        + 'static,
{
    func: F,
    info: FactoryTypeInfo,
    _phantom: std::marker::PhantomData<S>,
}

impl<S, F> FunctionNodeFactory<S, F>
where
    S: Send + Sync + 'static,
    F: Fn(S) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<S>> + Send>>
        + Send
        + Sync
        + Clone
        + 'static,
{
    /// Create a new function node factory
    pub fn new(name: impl Into<String>, func: F) -> Self {
        Self {
            func,
            info: FactoryTypeInfo::new(name).with_category("function"),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.info = self.info.with_description(description);
        self
    }

    /// Set category
    #[must_use]
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.info = self.info.with_category(category);
        self
    }

    /// Add tag
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.info = self.info.with_tag(tag);
        self
    }
}

impl<S, F> NodeFactory<S> for FunctionNodeFactory<S, F>
where
    S: Send + Sync + 'static,
    F: Fn(S) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<S>> + Send>>
        + Send
        + Sync
        + Clone
        + 'static,
{
    fn create(
        &self,
        _config: &serde_json::Value,
    ) -> std::result::Result<BoxedNode<S>, NodeFactoryError> {
        let func = self.func.clone();
        Ok(Arc::new(crate::node::FunctionNode::new(
            self.info.name.clone(),
            func,
        )))
    }

    fn type_info(&self) -> FactoryTypeInfo {
        self.info.clone()
    }
}

/// Factory for identity/pass-through nodes
pub struct IdentityNodeFactory {
    info: FactoryTypeInfo,
}

impl Default for IdentityNodeFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl IdentityNodeFactory {
    /// Create a new identity node factory
    #[must_use]
    pub fn new() -> Self {
        Self {
            info: FactoryTypeInfo::new("identity")
                .with_description("Pass-through node that returns state unchanged")
                .with_category("transform"),
        }
    }
}

impl<S> NodeFactory<S> for IdentityNodeFactory
where
    S: Clone + Send + Sync + 'static,
{
    fn create(
        &self,
        _config: &serde_json::Value,
    ) -> std::result::Result<BoxedNode<S>, NodeFactoryError> {
        Ok(Arc::new(IdentityNode::<S>::new()))
    }

    fn type_info(&self) -> FactoryTypeInfo {
        self.info.clone()
    }
}

/// Simple identity node that returns state unchanged
struct IdentityNode<S> {
    _phantom: std::marker::PhantomData<S>,
}

impl<S> IdentityNode<S> {
    fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<S> Node<S> for IdentityNode<S>
where
    S: Clone + Send + Sync + 'static,
{
    async fn execute(&self, state: S) -> Result<S> {
        Ok(state)
    }

    fn name(&self) -> String {
        "identity".to_string()
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct TestState {
        value: i32,
    }

    struct IncrementFactory {
        amount: i32,
    }

    impl NodeFactory<TestState> for IncrementFactory {
        fn create(
            &self,
            config: &serde_json::Value,
        ) -> std::result::Result<BoxedNode<TestState>, NodeFactoryError> {
            let amount = config
                .get("amount")
                .and_then(|v| v.as_i64())
                .unwrap_or(self.amount as i64) as i32;

            Ok(Arc::new(IncrementNode { amount }))
        }

        fn type_info(&self) -> FactoryTypeInfo {
            FactoryTypeInfo::new("increment")
                .with_description("Increments the value by a configurable amount")
                .with_category("transform")
                .with_config_schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "amount": {
                            "type": "integer",
                            "default": 1
                        }
                    }
                }))
                .with_example(serde_json::json!({"amount": 5}))
                .with_tag("math")
        }

        fn validate_config(
            &self,
            config: &serde_json::Value,
        ) -> std::result::Result<(), NodeFactoryError> {
            if let Some(amount) = config.get("amount") {
                if !amount.is_i64() && !amount.is_null() {
                    return Err(NodeFactoryError::InvalidConfig {
                        node_type: "increment".to_string(),
                        message: "amount must be an integer".to_string(),
                    });
                }
            }
            Ok(())
        }
    }

    struct IncrementNode {
        amount: i32,
    }

    #[async_trait::async_trait]
    impl Node<TestState> for IncrementNode {
        async fn execute(&self, state: TestState) -> Result<TestState> {
            Ok(TestState {
                value: state.value + self.amount,
            })
        }

        fn name(&self) -> String {
            format!("increment_{}", self.amount)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_node_type_info() {
        let info = FactoryTypeInfo::new("test.node")
            .with_description("A test node")
            .with_category("testing")
            .with_tag("test")
            .with_tag("example")
            .with_version("2.0.0");

        assert_eq!(info.name, "test.node");
        assert_eq!(info.description, "A test node");
        assert_eq!(info.category, "testing");
        assert_eq!(info.tags, vec!["test", "example"]);
        assert_eq!(info.version, "2.0.0");
        assert!(!info.deprecated);
    }

    #[test]
    fn test_node_type_info_deprecated() {
        let info = FactoryTypeInfo::new("old.node").deprecated("Use new.node instead");

        assert!(info.deprecated);
        assert_eq!(
            info.deprecation_message,
            Some("Use new.node instead".to_string())
        );
    }

    #[test]
    fn test_registry_new() {
        let registry: NodeRegistry<TestState> = NodeRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_register() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();

        let factory = IncrementFactory { amount: 1 };
        let prev = registry.register("increment", factory);

        assert!(prev.is_none());
        assert!(registry.contains("increment"));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_registry_register_overwrite() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();

        registry.register("increment", IncrementFactory { amount: 1 });
        let prev = registry.register("increment", IncrementFactory { amount: 5 });

        assert!(prev.is_some());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("increment", IncrementFactory { amount: 1 });

        let removed = registry.unregister("increment");
        assert!(removed.is_some());
        assert!(!registry.contains("increment"));
        assert!(registry.is_empty());
    }

    #[tokio::test]
    async fn test_registry_create() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("increment", IncrementFactory { amount: 1 });

        let config = serde_json::json!({"amount": 10});
        let node = registry.create("increment", &config).unwrap();

        let state = TestState { value: 5 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 15);
    }

    #[test]
    fn test_registry_create_unknown() {
        let registry: NodeRegistry<TestState> = NodeRegistry::new();

        let result = registry.create("unknown", &serde_json::json!({}));
        assert!(matches!(result, Err(NodeFactoryError::UnknownNodeType(_))));
    }

    #[test]
    fn test_registry_validate_config() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("increment", IncrementFactory { amount: 1 });

        // Valid config
        let valid = registry.validate_config("increment", &serde_json::json!({"amount": 5}));
        assert!(valid.is_ok());

        // Invalid config
        let invalid =
            registry.validate_config("increment", &serde_json::json!({"amount": "not a number"}));
        assert!(matches!(
            invalid,
            Err(NodeFactoryError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn test_registry_get_type_info() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("increment", IncrementFactory { amount: 1 });

        let info = registry.get_type_info("increment").unwrap();
        assert_eq!(info.name, "increment");
        assert_eq!(info.category, "transform");
    }

    #[test]
    fn test_registry_list_types() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("increment", IncrementFactory { amount: 1 });
        registry.register("identity", IdentityNodeFactory::new());

        let types = registry.list_types();
        assert_eq!(types.len(), 2);
    }

    #[test]
    fn test_registry_list_by_category() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("increment", IncrementFactory { amount: 1 });
        registry.register("identity", IdentityNodeFactory::new());

        let transform_types = registry.list_by_category("transform");
        assert_eq!(transform_types.len(), 2);
    }

    #[test]
    fn test_registry_find_by_tag() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("increment", IncrementFactory { amount: 1 });

        let math_types = registry.find_by_tag("math");
        assert_eq!(math_types.len(), 1);
        assert_eq!(math_types[0].name, "increment");
    }

    #[test]
    fn test_registry_search() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("increment", IncrementFactory { amount: 1 });
        registry.register("identity", IdentityNodeFactory::new());

        let results = registry.search("incr");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "increment");

        let results2 = registry.search("pass-through");
        assert_eq!(results2.len(), 1);
        assert_eq!(results2[0].name, "identity");
    }

    #[test]
    fn test_registry_categories() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("increment", IncrementFactory { amount: 1 });
        registry.register("identity", IdentityNodeFactory::new());

        let categories = registry.categories();
        assert_eq!(categories, vec!["transform"]);
    }

    #[test]
    fn test_registry_to_json() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("increment", IncrementFactory { amount: 1 });

        let json = registry.to_json().unwrap();
        assert!(json.contains("increment"));
        assert!(json.contains("transform"));
    }

    #[tokio::test]
    async fn test_identity_factory() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("identity", IdentityNodeFactory::new());

        let node = registry.create("identity", &serde_json::json!({})).unwrap();

        let state = TestState { value: 42 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 42);
    }

    #[tokio::test]
    async fn test_function_node_factory() {
        let factory = FunctionNodeFactory::new("double", |state: TestState| {
            Box::pin(async move {
                Ok(TestState {
                    value: state.value * 2,
                })
            })
        })
        .with_description("Doubles the value")
        .with_category("math")
        .with_tag("arithmetic");

        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("double", factory);

        let node = registry.create("double", &serde_json::json!({})).unwrap();
        let state = TestState { value: 7 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 14);

        let info = registry.get_type_info("double").unwrap();
        assert_eq!(info.description, "Doubles the value");
        assert_eq!(info.category, "math");
    }

    #[test]
    fn test_node_factory_error_display() {
        let err1 = NodeFactoryError::UnknownNodeType("foo".to_string());
        assert!(err1.to_string().contains("Unknown node type: 'foo'"));

        let err2 = NodeFactoryError::InvalidConfig {
            node_type: "bar".to_string(),
            message: "bad config".to_string(),
        };
        assert!(err2.to_string().contains("Invalid configuration"));

        let err3 = NodeFactoryError::CreationFailed {
            node_type: "baz".to_string(),
            message: "failed".to_string(),
        };
        assert!(err3.to_string().contains("Failed to create"));

        let err4 = NodeFactoryError::TypeMismatch {
            node_type: "qux".to_string(),
            expected: "State1".to_string(),
            actual: "State2".to_string(),
        };
        assert!(err4.to_string().contains("Type mismatch"));
    }

    #[test]
    fn test_registry_debug() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        registry.register("increment", IncrementFactory { amount: 1 });

        let debug_str = format!("{:?}", registry);
        assert!(debug_str.contains("NodeRegistry"));
        assert!(debug_str.contains("factory_count"));
    }
}
