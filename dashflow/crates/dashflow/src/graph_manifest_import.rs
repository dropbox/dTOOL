// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Graph Manifest Import - Dynamic Graph Construction from JSON
//!
//! This module enables AI agents to dynamically construct graphs from JSON manifests
//! using registered node factories. It bridges the gap between declarative graph
//! definitions and runtime graph construction.
//!
//! ## Key Components
//!
//! - [`ManifestImporter`]: Main builder for constructing graphs from manifests
//! - [`ConditionFactory`]: Trait for creating condition functions from config
//! - [`ConditionRegistry`]: Registry for condition factories (conditional edges)
//! - [`ManifestImportError`]: Error type for import failures
//!
//! ## Example: Basic Import
//!
//! ```rust,ignore
//! use dashflow::graph_manifest_import::{ManifestImporter, ManifestImportError};
//! use dashflow::node_registry::NodeRegistry;
//! use dashflow::introspection::GraphManifest;
//!
//! // Setup registries
//! let mut node_registry = NodeRegistry::new();
//! node_registry.register("identity", IdentityNodeFactory::new());
//!
//! // Parse manifest from JSON
//! let manifest: GraphManifest = serde_json::from_str(manifest_json)?;
//!
//! // Build graph
//! let graph = ManifestImporter::new(&node_registry)
//!     .import(&manifest)?;
//!
//! // Compile and run
//! let app = graph.compile()?;
//! ```
//!
//! ## Example: With Conditional Edges
//!
//! ```rust,ignore
//! use dashflow::graph_manifest_import::{ManifestImporter, ConditionRegistry, ConditionFactory};
//!
//! // Define a condition factory
//! struct ThresholdConditionFactory;
//!
//! impl<S: Clone + Send + Sync + 'static> ConditionFactory<S> for ThresholdConditionFactory
//! where
//!     S: HasValue,  // Custom trait to access a value
//! {
//!     fn create(&self, config: &serde_json::Value) -> Result<Box<dyn Fn(&S) -> String + Send + Sync>, ManifestImportError> {
//!         let threshold = config.get("threshold").and_then(|v| v.as_i64()).unwrap_or(50);
//!         Ok(Box::new(move |state: &S| {
//!             if state.value() > threshold { "high".to_string() } else { "low".to_string() }
//!         }))
//!     }
//! }
//!
//! // Register conditions
//! let mut condition_registry = ConditionRegistry::new();
//! condition_registry.register("threshold", Box::new(ThresholdConditionFactory));
//!
//! // Import with conditions
//! let graph = ManifestImporter::new(&node_registry)
//!     .with_conditions(&condition_registry)
//!     .import(&manifest)?;
//! ```
//!
//! ## JSON Manifest Format
//!
//! The `GraphManifest` format supports:
//!
//! ```json
//! {
//!   "entry_point": "start",
//!   "nodes": {
//!     "start": { "name": "start", "node_type": "function", ... },
//!     "process": { "name": "process", "node_type": "function", ... }
//!   },
//!   "edges": {
//!     "start": [{ "from": "start", "to": "process", "is_conditional": false }],
//!     "process": [{ "from": "process", "to": "__end__", "is_conditional": false }]
//!   },
//!   "node_configs": {
//!     "start": { "name": "start", "node_type": "identity", "config": {} },
//!     "process": { "name": "process", "node_type": "transform", "config": {"multiplier": 2} }
//!   }
//! }
//! ```

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use thiserror::Error;

use crate::edge::END;
use crate::introspection::{GraphManifest, NodeConfig};
use crate::node_registry::{NodeFactoryError, NodeRegistry};
use crate::registry_trait::Registry;
use crate::state::GraphState;
use crate::StateGraph;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during manifest import
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ManifestImportError {
    /// A node in the manifest has no corresponding factory
    #[error("No factory registered for node '{node_name}' of type '{node_type}'. Register a factory with NodeRegistry::register(\"{node_type}\", factory).")]
    MissingNodeFactory {
        /// Node name from the manifest that could not be constructed.
        node_name: String,
        /// Node type identifier from the manifest that was not registered.
        node_type: String,
    },
    /// Node factory failed to create the node
    #[error("Failed to create node '{node_name}' of type '{node_type}': {error}")]
    NodeCreationFailed {
        /// Node name from the manifest that failed to construct.
        node_name: String,
        /// Node type identifier from the manifest.
        node_type: String,
        /// Human-readable error message from the factory.
        error: String,
    },
    /// A conditional edge references an unknown condition type
    #[error("No condition factory registered for edge from '{edge_from}' with type '{condition_type}'. Register a factory with ConditionRegistry::register(\"{condition_type}\", factory).")]
    MissingConditionFactory {
        /// Source node name of the conditional edge.
        edge_from: String,
        /// Condition type identifier referenced by the manifest.
        condition_type: String,
    },
    /// Condition factory failed to create the condition
    #[error("Failed to create condition for edge from '{edge_from}' of type '{condition_type}': {error}")]
    ConditionCreationFailed {
        /// Source node name of the conditional edge.
        edge_from: String,
        /// Condition type identifier referenced by the manifest.
        condition_type: String,
        /// Human-readable error message from the factory.
        error: String,
    },
    /// Entry point node not found in manifest
    #[error("Entry point '{entry_point}' not found in manifest nodes. Add a node_config for '{entry_point}' or check the entry_point field.")]
    MissingEntryPoint {
        /// Entry point node name referenced by the manifest.
        entry_point: String,
    },
    /// Edge references unknown node
    #[error("Edge from '{from}' to '{to}' references unknown node. Add a node_config for '{to}' or use '__end__' for termination.")]
    UnknownEdgeTarget {
        /// Source node name of the edge.
        from: String,
        /// Target node name of the edge.
        to: String,
    },
    /// Node config references unknown node
    #[error("Node config '{config_name}' mismatch: {message}")]
    NodeConfigMismatch {
        /// Name of the node config entry being processed.
        config_name: String,
        /// Human-readable reason the config was rejected.
        message: String,
    },
    /// Manifest validation failed
    #[error("Manifest validation failed: {message}")]
    ValidationFailed {
        /// Human-readable reason the manifest was rejected.
        message: String,
    },
    /// JSON parsing error
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
}

impl From<NodeFactoryError> for ManifestImportError {
    fn from(err: NodeFactoryError) -> Self {
        match err {
            NodeFactoryError::UnknownNodeType(name) => Self::MissingNodeFactory {
                node_name: String::new(),
                node_type: name,
            },
            NodeFactoryError::InvalidConfig { node_type, message } => Self::NodeCreationFailed {
                node_name: String::new(),
                node_type,
                error: message,
            },
            NodeFactoryError::CreationFailed { node_type, message } => Self::NodeCreationFailed {
                node_name: String::new(),
                node_type,
                error: message,
            },
            NodeFactoryError::TypeMismatch {
                node_type,
                expected,
                actual,
            } => Self::NodeCreationFailed {
                node_name: String::new(),
                node_type,
                error: format!("Type mismatch: expected {}, got {}", expected, actual),
            },
        }
    }
}

// ============================================================================
// Condition Factory - For Dynamic Conditional Edges
// ============================================================================

/// Factory trait for creating condition functions from configuration
///
/// Implement this trait to enable conditional edges in JSON manifests.
/// Each implementation knows how to create a specific type of condition
/// from a JSON configuration.
///
/// # Example
///
/// ```rust,ignore
/// struct FieldCompareFactory;
///
/// impl<S> ConditionFactory<S> for FieldCompareFactory
/// where
///     S: HasField + Clone + Send + Sync + 'static,
/// {
///     fn create(
///         &self,
///         config: &serde_json::Value,
///     ) -> Result<ConditionFn<S>, ManifestImportError> {
///         let field = config.get("field").and_then(|v| v.as_str()).unwrap_or("value");
///         let threshold = config.get("threshold").and_then(|v| v.as_i64()).unwrap_or(0);
///         let field = field.to_string();
///
///         Ok(Box::new(move |state: &S| {
///             if state.get_field(&field) > threshold { "high" } else { "low" }.to_string()
///         }))
///     }
///
///     fn condition_type(&self) -> &str {
///         "field_compare"
///     }
/// }
/// ```
pub trait ConditionFactory<S>: Send + Sync
where
    S: Send + Sync + 'static,
{
    /// Create a condition function from configuration
    ///
    /// # Arguments
    /// * `config` - JSON configuration for the condition
    ///
    /// # Returns
    /// A boxed condition function that takes state and returns a route key
    fn create(&self, config: &serde_json::Value) -> Result<ConditionFn<S>, ManifestImportError>;

    /// Get the condition type identifier
    fn condition_type(&self) -> &str;
}

/// Type alias for condition functions
pub type ConditionFn<S> = Box<dyn Fn(&S) -> String + Send + Sync>;

// ============================================================================
// Condition Registry
// ============================================================================

/// Registry for condition factories
///
/// The condition registry stores factories indexed by condition type name,
/// enabling dynamic creation of conditional edges from JSON configuration.
///
/// # Example
///
/// ```rust,ignore
/// let mut registry = ConditionRegistry::new();
/// registry.register("threshold", ThresholdConditionFactory);
/// registry.register("field_match", FieldMatchConditionFactory);
///
/// // Create a condition from config
/// let config = json!({"threshold": 50, "high_route": "fast", "low_route": "slow"});
/// let condition = registry.create("threshold", &config)?;
/// ```
pub struct ConditionRegistry<S>
where
    S: Send + Sync + 'static,
{
    factories: HashMap<String, Arc<dyn ConditionFactory<S>>>,
}

impl<S> Default for ConditionRegistry<S>
where
    S: Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> ConditionRegistry<S>
where
    S: Send + Sync + 'static,
{
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Register a condition factory
    ///
    /// # Arguments
    /// * `condition_type` - Unique identifier for this condition type
    /// * `factory` - Factory implementation
    pub fn register<F>(&mut self, condition_type: impl Into<String>, factory: F)
    where
        F: ConditionFactory<S> + 'static,
    {
        self.factories
            .insert(condition_type.into(), Arc::new(factory));
    }

    /// Check if a condition type is registered
    #[must_use]
    pub fn contains(&self, condition_type: &str) -> bool {
        self.factories.contains_key(condition_type)
    }

    /// Create a condition function from configuration
    ///
    /// # Arguments
    /// * `condition_type` - Type of condition to create
    /// * `config` - Configuration for the condition
    ///
    /// # Errors
    /// Returns error if condition type is not registered or creation fails
    pub fn create(
        &self,
        condition_type: &str,
        config: &serde_json::Value,
    ) -> Result<ConditionFn<S>, ManifestImportError> {
        let factory = self.factories.get(condition_type).ok_or_else(|| {
            ManifestImportError::MissingConditionFactory {
                edge_from: String::new(),
                condition_type: condition_type.to_string(),
            }
        })?;

        factory.create(config)
    }

    /// List all registered condition types
    #[must_use]
    pub fn list_types(&self) -> Vec<&str> {
        self.factories.keys().map(String::as_str).collect()
    }
}

impl<S> fmt::Debug for ConditionRegistry<S>
where
    S: Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConditionRegistry")
            .field("factory_count", &self.factories.len())
            .field("types", &self.factories.keys().collect::<Vec<_>>())
            .finish()
    }
}

// ============================================================================
// Registry Trait Implementation (Phase 2.2 of REFACTORING_PLAN.md)
// ============================================================================

/// Implements the standard Registry trait for ConditionRegistry.
impl<S> Registry<Arc<dyn ConditionFactory<S>>> for ConditionRegistry<S>
where
    S: Send + Sync + 'static,
{
    fn get(&self, key: &str) -> Option<&Arc<dyn ConditionFactory<S>>> {
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
// Extended Edge Manifest for Conditions
// ============================================================================

/// Extended edge configuration for conditional edges
///
/// This structure extends `EdgeManifest` with condition configuration
/// needed for dynamic conditional edge creation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConditionalEdgeConfig {
    /// Source node name
    pub from: String,
    /// Condition type (maps to factory in ConditionRegistry)
    pub condition_type: String,
    /// Configuration for the condition factory
    pub condition_config: serde_json::Value,
    /// Route mapping: condition return value -> target node
    pub routes: HashMap<String, String>,
}

// ============================================================================
// Manifest Importer - Main Entry Point
// ============================================================================

/// Builder for importing graphs from manifests
///
/// `ManifestImporter` provides a fluent API for constructing graphs from
/// `GraphManifest` instances using registered factories.
///
/// # Type Parameters
///
/// * `S` - The state type for the graph
///
/// # Example
///
/// ```rust,ignore
/// let graph = ManifestImporter::new(&node_registry)
///     .with_conditions(&condition_registry)  // Optional: for conditional edges
///     .strict()                               // Optional: fail on warnings
///     .import(&manifest)?;
/// ```
pub struct ManifestImporter<'a, S>
where
    S: GraphState,
{
    node_registry: &'a NodeRegistry<S>,
    condition_registry: Option<&'a ConditionRegistry<S>>,
    strict: bool,
    skip_unknown_nodes: bool,
    conditional_edges: Vec<ConditionalEdgeConfig>,
}

impl<'a, S> ManifestImporter<'a, S>
where
    S: GraphState,
{
    /// Create a new manifest importer with a node registry
    ///
    /// # Arguments
    /// * `node_registry` - Registry containing node factories
    #[must_use]
    pub fn new(node_registry: &'a NodeRegistry<S>) -> Self {
        Self {
            node_registry,
            condition_registry: None,
            strict: false,
            skip_unknown_nodes: false,
            conditional_edges: Vec::new(),
        }
    }

    /// Add a condition registry for conditional edges
    ///
    /// Without a condition registry, conditional edges in the manifest
    /// will be converted to simple edges (with a warning unless strict mode).
    #[must_use]
    pub fn with_conditions(mut self, registry: &'a ConditionRegistry<S>) -> Self {
        self.condition_registry = Some(registry);
        self
    }

    /// Enable strict mode
    ///
    /// In strict mode, warnings become errors:
    /// - Missing node factories cause import to fail
    /// - Conditional edges without condition registry cause import to fail
    #[must_use]
    pub fn strict(mut self) -> Self {
        self.strict = true;
        self
    }

    /// Skip nodes with unknown types instead of failing
    ///
    /// When enabled, nodes without a registered factory are skipped
    /// (with a warning). Edges to/from skipped nodes are also skipped.
    #[must_use]
    pub fn skip_unknown_nodes(mut self) -> Self {
        self.skip_unknown_nodes = true;
        self
    }

    /// Add conditional edge configurations
    ///
    /// These are processed during import to create conditional edges.
    /// Use this when the manifest's `edges` field contains conditional
    /// edges that need factory-based condition creation.
    #[must_use]
    pub fn with_conditional_edges(mut self, edges: Vec<ConditionalEdgeConfig>) -> Self {
        self.conditional_edges = edges;
        self
    }

    /// Import a graph from a manifest
    ///
    /// This creates a `StateGraph` by:
    /// 1. Creating nodes from `node_configs` using the node registry
    /// 2. Adding edges from the manifest
    /// 3. Setting the entry point
    ///
    /// # Arguments
    /// * `manifest` - The graph manifest to import
    ///
    /// # Returns
    /// A configured `StateGraph` ready for compilation
    ///
    /// # Errors
    /// - `MissingNodeFactory`: Node type not in registry (unless `skip_unknown_nodes`)
    /// - `NodeCreationFailed`: Factory failed to create node
    /// - `MissingEntryPoint`: Entry point not found
    /// - `UnknownEdgeTarget`: Edge references unknown node
    pub fn import(self, manifest: &GraphManifest) -> Result<StateGraph<S>, ManifestImportError> {
        let mut graph = StateGraph::new();
        let mut created_nodes = std::collections::HashSet::new();

        // Step 1: Create nodes from node_configs
        for (name, config) in &manifest.node_configs {
            match self.create_node(name, config) {
                Ok(node) => {
                    graph.add_boxed_node(name, node);
                    created_nodes.insert(name.clone());
                }
                Err(e) => {
                    if self.skip_unknown_nodes
                        && matches!(e, ManifestImportError::MissingNodeFactory { .. })
                    {
                        tracing::warn!(
                            node_name = %name,
                            error = %e,
                            "Skipping node with unknown type"
                        );
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        // Step 2: Validate entry point exists
        if !created_nodes.contains(&manifest.entry_point) {
            // Try to create entry point node if it's in nodes but not node_configs
            if manifest.nodes.contains_key(&manifest.entry_point) {
                return Err(ManifestImportError::NodeConfigMismatch {
                    config_name: manifest.entry_point.clone(),
                    message: format!(
                        "Entry point '{}' exists in nodes but has no node_config. \
                         Add a node_config to specify how to create this node.",
                        manifest.entry_point
                    ),
                });
            }
            return Err(ManifestImportError::MissingEntryPoint {
                entry_point: manifest.entry_point.clone(),
            });
        }

        // Step 3: Add edges
        for (from, edges) in &manifest.edges {
            // Skip edges from nodes we didn't create
            if !created_nodes.contains(from) && self.skip_unknown_nodes {
                continue;
            }

            for edge in edges {
                // Skip edges to nodes we didn't create (unless it's END)
                if edge.to != END && !created_nodes.contains(&edge.to) {
                    if self.skip_unknown_nodes {
                        tracing::warn!(
                            from = %from,
                            to = %edge.to,
                            "Skipping edge to unknown node"
                        );
                        continue;
                    }
                    return Err(ManifestImportError::UnknownEdgeTarget {
                        from: from.clone(),
                        to: edge.to.clone(),
                    });
                }

                // Handle conditional edges
                if edge.is_conditional {
                    if self.condition_registry.is_none() {
                        if self.strict {
                            return Err(ManifestImportError::ValidationFailed {
                                message: format!(
                                    "Conditional edge from '{}' requires a condition registry. \
                                     Use .with_conditions() or convert to simple edge.",
                                    from
                                ),
                            });
                        }
                        tracing::warn!(
                            from = %from,
                            to = %edge.to,
                            "Converting conditional edge to simple edge (no condition registry)"
                        );
                        graph.add_edge(from, &edge.to);
                        continue;
                    }
                    // Conditional edges from EdgeManifest don't have condition config
                    // They need to be in conditional_edges with full config
                    tracing::warn!(
                        from = %from,
                        to = %edge.to,
                        "Conditional edge in manifest.edges lacks condition config. \
                         Converting to simple edge. Use conditional_edges for full support."
                    );
                    graph.add_edge(from, &edge.to);
                } else {
                    graph.add_edge(from, &edge.to);
                }
            }
        }

        // Step 4: Process explicit conditional edge configs
        for cond_edge in &self.conditional_edges {
            if !created_nodes.contains(&cond_edge.from) {
                if self.skip_unknown_nodes {
                    continue;
                }
                return Err(ManifestImportError::UnknownEdgeTarget {
                    from: cond_edge.from.clone(),
                    to: "conditional".to_string(),
                });
            }

            // Validate route targets
            for target in cond_edge.routes.values() {
                if target != END && !created_nodes.contains(target) {
                    if self.skip_unknown_nodes {
                        tracing::warn!(
                            from = %cond_edge.from,
                            to = %target,
                            "Skipping conditional route to unknown node"
                        );
                        continue;
                    }
                    return Err(ManifestImportError::UnknownEdgeTarget {
                        from: cond_edge.from.clone(),
                        to: target.clone(),
                    });
                }
            }

            // Create condition
            if let Some(registry) = self.condition_registry {
                let condition = registry
                    .create(&cond_edge.condition_type, &cond_edge.condition_config)
                    .map_err(|e| ManifestImportError::ConditionCreationFailed {
                        edge_from: cond_edge.from.clone(),
                        condition_type: cond_edge.condition_type.clone(),
                        error: e.to_string(),
                    })?;

                graph.add_conditional_edges(
                    &cond_edge.from,
                    move |state| condition(state),
                    cond_edge.routes.clone(),
                );
            } else {
                return Err(ManifestImportError::MissingConditionFactory {
                    edge_from: cond_edge.from.clone(),
                    condition_type: cond_edge.condition_type.clone(),
                });
            }
        }

        // Step 5: Set entry point
        graph.set_entry_point(&manifest.entry_point);

        // Step 6: Copy node configs for runtime access
        for (name, config) in &manifest.node_configs {
            if created_nodes.contains(name) {
                graph.insert_node_config(name, config.clone());
            }
        }

        Ok(graph)
    }

    /// Import a graph from JSON string
    ///
    /// Convenience method that parses JSON and imports in one step.
    ///
    /// # Arguments
    /// * `json` - JSON string representing a `GraphManifest`
    ///
    /// # Errors
    /// - JSON parsing errors
    /// - All errors from `import()`
    pub fn import_json(self, json: &str) -> Result<StateGraph<S>, ManifestImportError> {
        let manifest: GraphManifest = serde_json::from_str(json)?;
        self.import(&manifest)
    }

    /// Create a single node from config
    fn create_node(
        &self,
        name: &str,
        config: &NodeConfig,
    ) -> Result<crate::node::BoxedNode<S>, ManifestImportError> {
        // Check if factory exists
        if !self.node_registry.contains(&config.node_type) {
            return Err(ManifestImportError::MissingNodeFactory {
                node_name: name.to_string(),
                node_type: config.node_type.clone(),
            });
        }

        // Create node
        self.node_registry
            .create(&config.node_type, &config.config)
            .map_err(|e| ManifestImportError::NodeCreationFailed {
                node_name: name.to_string(),
                node_type: config.node_type.clone(),
                error: e.to_string(),
            })
    }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Import a graph from JSON string using a node registry
///
/// This is a convenience function for simple imports without
/// conditional edges.
///
/// # Arguments
/// * `json` - JSON string representing a `GraphManifest`
/// * `registry` - Node registry with registered factories
///
/// # Returns
/// A configured `StateGraph` ready for compilation
///
/// # Errors
/// - JSON parsing errors
/// - Factory not found for node type
/// - Node creation failures
///
/// # Example
///
/// ```rust,ignore
/// let graph = from_json::<MyState>(manifest_json, &registry)?;
/// let app = graph.compile()?;
/// ```
pub fn from_json<S>(
    json: &str,
    registry: &NodeRegistry<S>,
) -> Result<StateGraph<S>, ManifestImportError>
where
    S: GraphState,
{
    ManifestImporter::new(registry).import_json(json)
}

/// Import a graph from a manifest using a node registry
///
/// This is a convenience function for simple imports without
/// conditional edges.
///
/// # Arguments
/// * `manifest` - The graph manifest
/// * `registry` - Node registry with registered factories
///
/// # Returns
/// A configured `StateGraph` ready for compilation
///
/// # Errors
/// - Factory not found for node type
/// - Node creation failures
///
/// # Example
///
/// ```rust,ignore
/// let manifest = GraphManifest::from_json(json)?;
/// let graph = from_manifest::<MyState>(&manifest, &registry)?;
/// let app = graph.compile()?;
/// ```
pub fn from_manifest<S>(
    manifest: &GraphManifest,
    registry: &NodeRegistry<S>,
) -> Result<StateGraph<S>, ManifestImportError>
where
    S: GraphState,
{
    ManifestImporter::new(registry).import(manifest)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{EdgeManifest, NodeConfig, NodeManifest, NodeType};
    use crate::node_registry::{FactoryTypeInfo, IdentityNodeFactory, NodeFactory};
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct TestState {
        value: i32,
    }

    // GraphState is automatically implemented via blanket impl
    // (for any T: Clone + Send + Sync + Serialize + Deserialize + 'static)

    impl crate::state::MergeableState for TestState {
        fn merge(&mut self, other: &Self) {
            self.value = other.value;
        }
    }

    // Test factory that creates identity nodes
    struct TestNodeFactory;

    impl NodeFactory<TestState> for TestNodeFactory {
        fn create(
            &self,
            _config: &serde_json::Value,
        ) -> std::result::Result<
            crate::node::BoxedNode<TestState>,
            crate::node_registry::NodeFactoryError,
        > {
            Ok(std::sync::Arc::new(TestNode))
        }

        fn type_info(&self) -> FactoryTypeInfo {
            FactoryTypeInfo::new("test")
                .with_description("Test node")
                .with_category("test")
        }
    }

    struct TestNode;

    #[async_trait::async_trait]
    impl crate::Node<TestState> for TestNode {
        async fn execute(&self, state: TestState) -> crate::error::Result<TestState> {
            Ok(state)
        }

        fn name(&self) -> String {
            "test".to_string()
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    fn create_test_registry() -> NodeRegistry<TestState> {
        let mut registry = NodeRegistry::new();
        registry.register("test", TestNodeFactory);
        registry.register("identity", IdentityNodeFactory::new());
        registry
    }

    fn create_test_manifest() -> GraphManifest {
        GraphManifest::builder()
            .entry_point("start")
            .add_node("start", NodeManifest::new("start", NodeType::Function))
            .add_node("process", NodeManifest::new("process", NodeType::Function))
            .add_edge("start", EdgeManifest::simple("start", "process"))
            .add_edge("process", EdgeManifest::simple("process", END))
            .add_node_config(
                "start",
                NodeConfig::new("start", "identity").with_config(serde_json::json!({})),
            )
            .add_node_config(
                "process",
                NodeConfig::new("process", "test").with_config(serde_json::json!({})),
            )
            .build()
            .unwrap()
    }

    #[test]
    fn test_manifest_import_basic() {
        let registry = create_test_registry();
        let manifest = create_test_manifest();

        let result = ManifestImporter::new(&registry).import(&manifest);
        assert!(result.is_ok(), "Import failed: {:?}", result.err());

        let graph = result.unwrap();
        assert!(graph.has_node("start"));
        assert!(graph.has_node("process"));
    }

    #[test]
    fn test_manifest_import_from_json() {
        let registry = create_test_registry();
        let manifest = create_test_manifest();
        let json = manifest.to_json().unwrap();

        let result = ManifestImporter::new(&registry).import_json(&json);
        assert!(result.is_ok(), "Import failed: {:?}", result.err());
    }

    #[test]
    fn test_manifest_import_missing_factory() {
        let registry: NodeRegistry<TestState> = NodeRegistry::new(); // Empty registry
        let manifest = create_test_manifest();

        let result = ManifestImporter::new(&registry).import(&manifest);
        assert!(matches!(
            result,
            Err(ManifestImportError::MissingNodeFactory { .. })
        ));
    }

    #[test]
    fn test_manifest_import_skip_unknown_nodes() {
        let mut registry: NodeRegistry<TestState> = NodeRegistry::new();
        // Only register identity, not test
        registry.register("identity", IdentityNodeFactory::new());

        let manifest = create_test_manifest();

        let result = ManifestImporter::new(&registry)
            .skip_unknown_nodes()
            .import(&manifest);

        // Should succeed, skipping the 'process' node
        assert!(result.is_ok(), "Import failed: {:?}", result.err());

        let graph = result.unwrap();
        assert!(graph.has_node("start"));
        // process was skipped because 'test' factory not registered
        assert!(!graph.has_node("process"));
    }

    #[test]
    fn test_manifest_import_missing_entry_point() {
        let registry = create_test_registry();

        // Create manifest with entry point not in node_configs
        let manifest = GraphManifest::builder()
            .entry_point("nonexistent")
            .add_node("start", NodeManifest::new("start", NodeType::Function))
            .add_node_config(
                "start",
                NodeConfig::new("start", "identity").with_config(serde_json::json!({})),
            )
            .build()
            .unwrap();

        let result = ManifestImporter::new(&registry).import(&manifest);
        assert!(matches!(
            result,
            Err(ManifestImportError::MissingEntryPoint { .. })
        ));
    }

    #[test]
    fn test_convenience_from_json() {
        let registry = create_test_registry();
        let manifest = create_test_manifest();
        let json = manifest.to_json().unwrap();

        let result = from_json::<TestState>(&json, &registry);
        assert!(result.is_ok(), "Import failed: {:?}", result.err());
    }

    #[test]
    fn test_convenience_from_manifest() {
        let registry = create_test_registry();
        let manifest = create_test_manifest();

        let result = from_manifest(&manifest, &registry);
        assert!(result.is_ok(), "Import failed: {:?}", result.err());
    }

    #[test]
    fn test_condition_registry() {
        struct AlwaysHighCondition;

        impl ConditionFactory<TestState> for AlwaysHighCondition {
            fn create(
                &self,
                _config: &serde_json::Value,
            ) -> Result<ConditionFn<TestState>, ManifestImportError> {
                Ok(Box::new(|_state: &TestState| "high".to_string()))
            }

            fn condition_type(&self) -> &str {
                "always_high"
            }
        }

        let mut registry: ConditionRegistry<TestState> = ConditionRegistry::new();
        registry.register("always_high", AlwaysHighCondition);

        assert!(registry.contains("always_high"));
        assert!(!registry.contains("nonexistent"));

        let condition = registry.create("always_high", &serde_json::json!({}));
        assert!(condition.is_ok());

        let condition_fn = condition.unwrap();
        assert_eq!(condition_fn(&TestState { value: 42 }), "high");
    }

    #[test]
    fn test_manifest_import_error_display() {
        let err1 = ManifestImportError::MissingNodeFactory {
            node_name: "my_node".to_string(),
            node_type: "custom".to_string(),
        };
        let msg = err1.to_string();
        assert!(msg.contains("No factory registered"));
        assert!(msg.contains("my_node"));
        assert!(msg.contains("custom"));

        let err2 = ManifestImportError::MissingEntryPoint {
            entry_point: "start".to_string(),
        };
        let msg2 = err2.to_string();
        assert!(msg2.contains("Entry point"));
        assert!(msg2.contains("start"));
    }

    #[test]
    fn test_conditional_edge_config() {
        let config = ConditionalEdgeConfig {
            from: "decide".to_string(),
            condition_type: "threshold".to_string(),
            condition_config: serde_json::json!({"threshold": 50}),
            routes: {
                let mut m = HashMap::new();
                m.insert("high".to_string(), "fast_path".to_string());
                m.insert("low".to_string(), "slow_path".to_string());
                m
            },
        };

        assert_eq!(config.from, "decide");
        assert_eq!(config.condition_type, "threshold");
        assert_eq!(config.routes.len(), 2);
    }

    #[tokio::test]
    async fn test_imported_graph_compiles() {
        let registry = create_test_registry();
        let manifest = create_test_manifest();

        let graph = ManifestImporter::new(&registry).import(&manifest).unwrap();

        // Should compile without errors
        let result = graph.compile();
        assert!(result.is_ok(), "Compile failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_imported_graph_executes() {
        let registry = create_test_registry();
        let manifest = create_test_manifest();

        let graph = ManifestImporter::new(&registry).import(&manifest).unwrap();

        let app = graph.compile().unwrap();
        let initial_state = TestState { value: 42 };

        let result = app.invoke(initial_state).await;
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
        assert_eq!(result.unwrap().final_state.value, 42);
    }
}
