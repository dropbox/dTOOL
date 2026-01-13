// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Platform Introspection - DashFlow Framework Capabilities
//!
//! This module provides platform-level introspection for the DashFlow framework.
//! Unlike app-level introspection which describes a specific compiled graph,
//! platform introspection describes the DashFlow framework itself - its version,
//! available features, supported node types, edge types, and built-in templates.
//!
//! ## Three-Level Introspection Model
//!
//! DashFlow provides three levels of introspection:
//!
//! 1. **Platform Introspection** (this module) - DashFlow framework capabilities
//!    - Shared by ALL DashFlow applications
//!    - Version info, features, node types, edge types, templates
//!
//! 2. **App Introspection** - Application-specific configuration
//!    - Specific to ONE compiled graph
//!    - Graph structure, configured features, tools, state schema
//!
//! 3. **Live Introspection** - Runtime execution state (future)
//!    - Per-execution instance
//!    - Active executions, current node, state values, history
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::platform_introspection::PlatformIntrospection;
//!
//! // Get platform capabilities
//! let platform = PlatformIntrospection::discover();
//!
//! // Check version
//! println!("DashFlow version: {}", platform.version_info().version);
//!
//! // List available features
//! for feature in platform.available_features() {
//!     println!("{}: {}", feature.name, feature.description);
//! }
//!
//! // Query capabilities
//! if let Some(cap) = platform.query_capability("checkpointing") {
//!     println!("Checkpointing: {}", cap.description);
//! }
//!
//! // Export for AI consumption
//! let json = platform.to_json();
//! ```

use serde::{Deserialize, Serialize};

// Re-export the unified FeatureInfo from platform_registry
pub use crate::platform_registry::FeatureInfo;

// ============================================================================
// Version Information
// ============================================================================

/// Version information for the DashFlow framework.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionInfo {
    /// DashFlow version (e.g., "1.11.3")
    pub version: String,
    /// Minimum supported Rust version
    pub rust_version: String,
    /// Cargo features enabled at compile time
    pub features_enabled: Vec<String>,
    /// Build timestamp (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_timestamp: Option<String>,
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            rust_version: "1.75.0".to_string(), // MSRV
            features_enabled: Vec::new(),
            build_timestamp: None,
        }
    }
}

// ============================================================================
// Node Type Information
// ============================================================================

/// Information about a supported node type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeTypeInfo {
    /// Node type name (e.g., "function", "agent")
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Example usage code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    /// Whether this is a built-in type
    pub is_builtin: bool,
}

impl NodeTypeInfo {
    /// Create a new node type info.
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            example: None,
            is_builtin: true,
        }
    }

    /// Add an example.
    #[must_use]
    pub fn with_example(mut self, example: impl Into<String>) -> Self {
        self.example = Some(example.into());
        self
    }

    /// Mark as custom (not built-in).
    #[must_use]
    pub fn custom(mut self) -> Self {
        self.is_builtin = false;
        self
    }
}

// ============================================================================
// Edge Type Information
// ============================================================================

/// Information about a supported edge type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EdgeTypeInfo {
    /// Edge type name (e.g., "simple", "conditional")
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Example usage code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

impl EdgeTypeInfo {
    /// Create a new edge type info.
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            example: None,
        }
    }

    /// Add an example.
    #[must_use]
    pub fn with_example(mut self, example: impl Into<String>) -> Self {
        self.example = Some(example.into());
        self
    }
}

// ============================================================================
// Template Information
// ============================================================================

/// Information about a built-in graph template.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TemplateInfo {
    /// Template name (e.g., "supervisor", "react_agent")
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Use cases for this template
    pub use_cases: Vec<String>,
    /// Example usage code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

impl TemplateInfo {
    /// Create a new template info.
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            use_cases: Vec::new(),
            example: None,
        }
    }

    /// Add a use case.
    #[must_use]
    pub fn with_use_case(mut self, use_case: impl Into<String>) -> Self {
        self.use_cases.push(use_case.into());
        self
    }

    /// Add an example.
    #[must_use]
    pub fn with_example(mut self, example: impl Into<String>) -> Self {
        self.example = Some(example.into());
        self
    }
}

// ============================================================================
// State Type Information
// ============================================================================

/// Information about a `MergeableState` implementation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateTypeInfo {
    /// State type name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Whether this is a built-in type
    pub is_builtin: bool,
}

impl StateTypeInfo {
    /// Create a new state type info.
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            is_builtin: true,
        }
    }

    /// Mark as custom (not built-in).
    #[must_use]
    pub fn custom(mut self) -> Self {
        self.is_builtin = false;
        self
    }
}

// ============================================================================
// Capability Information (for queries)
// ============================================================================

/// Information about a platform capability (used for queries).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityInfo {
    /// Capability name
    pub name: String,
    /// Category (e.g., "feature", "node_type", "edge_type")
    pub category: String,
    /// Human-readable description
    pub description: String,
    /// Whether it's enabled/available
    pub available: bool,
}

impl CapabilityInfo {
    /// Create a new capability info.
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
            available: true,
        }
    }

    /// Mark as unavailable.
    #[must_use]
    pub fn unavailable(mut self) -> Self {
        self.available = false;
        self
    }
}

// ============================================================================
// Platform Introspection
// ============================================================================

/// Platform-level introspection for the DashFlow framework.
///
/// This structure provides comprehensive information about the DashFlow
/// framework's capabilities, independent of any specific application.
/// All DashFlow applications share this information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformIntrospection {
    /// Version information
    version_info: VersionInfo,
    /// Available features
    features: Vec<FeatureInfo>,
    /// Supported node types
    node_types: Vec<NodeTypeInfo>,
    /// Supported edge types
    edge_types: Vec<EdgeTypeInfo>,
    /// Built-in templates
    templates: Vec<TemplateInfo>,
    /// Available state implementations
    state_types: Vec<StateTypeInfo>,
}

impl Default for PlatformIntrospection {
    fn default() -> Self {
        Self::discover()
    }
}

impl PlatformIntrospection {
    /// Discover platform capabilities.
    ///
    /// Builds a complete registry of DashFlow framework capabilities.
    #[must_use]
    pub fn discover() -> Self {
        Self {
            version_info: Self::build_version_info(),
            features: Self::build_features(),
            node_types: Self::build_node_types(),
            edge_types: Self::build_edge_types(),
            templates: Self::build_templates(),
            state_types: Self::build_state_types(),
        }
    }

    /// Get DashFlow version information.
    #[must_use]
    pub fn version_info(&self) -> &VersionInfo {
        &self.version_info
    }

    /// List all available features.
    #[must_use]
    pub fn available_features(&self) -> &[FeatureInfo] {
        &self.features
    }

    /// List supported node types.
    #[must_use]
    pub fn supported_node_types(&self) -> &[NodeTypeInfo] {
        &self.node_types
    }

    /// List supported edge types.
    #[must_use]
    pub fn supported_edge_types(&self) -> &[EdgeTypeInfo] {
        &self.edge_types
    }

    /// List built-in templates.
    #[must_use]
    pub fn built_in_templates(&self) -> &[TemplateInfo] {
        &self.templates
    }

    /// List available `MergeableState` implementations.
    #[must_use]
    pub fn state_implementations(&self) -> &[StateTypeInfo] {
        &self.state_types
    }

    /// Query platform capabilities by name.
    ///
    /// Searches across features, node types, edge types, and templates.
    #[must_use]
    pub fn query_capability(&self, name: &str) -> Option<CapabilityInfo> {
        let name_lower = name.to_lowercase();

        // Search features
        if let Some(f) = self
            .features
            .iter()
            .find(|f| f.name.to_lowercase() == name_lower)
        {
            return Some(CapabilityInfo::new(&f.name, "feature", &f.description));
        }

        // Search node types
        if let Some(n) = self
            .node_types
            .iter()
            .find(|n| n.name.to_lowercase() == name_lower)
        {
            return Some(CapabilityInfo::new(&n.name, "node_type", &n.description));
        }

        // Search edge types
        if let Some(e) = self
            .edge_types
            .iter()
            .find(|e| e.name.to_lowercase() == name_lower)
        {
            return Some(CapabilityInfo::new(&e.name, "edge_type", &e.description));
        }

        // Search templates
        if let Some(t) = self
            .templates
            .iter()
            .find(|t| t.name.to_lowercase() == name_lower)
        {
            return Some(CapabilityInfo::new(&t.name, "template", &t.description));
        }

        // Search state types
        if let Some(s) = self
            .state_types
            .iter()
            .find(|s| s.name.to_lowercase() == name_lower)
        {
            return Some(CapabilityInfo::new(&s.name, "state_type", &s.description));
        }

        None
    }

    /// Export as JSON for AI consumption.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Build version info from compile-time constants.
    #[allow(clippy::vec_init_then_push, unused_mut)] // Conditional compilation requires push pattern
    fn build_version_info() -> VersionInfo {
        let mut features_enabled = Vec::new();

        // Check for common features
        #[cfg(feature = "dashstream")]
        features_enabled.push("dashstream".to_string());

        #[cfg(feature = "mcp-server")]
        features_enabled.push("mcp-server".to_string());

        VersionInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            rust_version: "1.75.0".to_string(),
            features_enabled,
            build_timestamp: option_env!("BUILD_TIMESTAMP").map(String::from),
        }
    }

    /// Build the list of available features.
    ///
    /// Delegates to `PlatformRegistry::discover()` to avoid duplicate hardcoding.
    /// This establishes PlatformRegistry as the single source of truth for features.
    /// (M-602: Replaced hardcoded features list with registry delegation)
    fn build_features() -> Vec<FeatureInfo> {
        // Pull from PlatformRegistry - the authoritative source for feature metadata
        crate::platform_registry::PlatformRegistry::discover().features
    }

    /// Build the list of supported node types.
    ///
    /// Delegates to `platform_registry::canonical_node_types()` for single source of truth.
    /// (M-603: Replaced hardcoded node types list with registry delegation)
    fn build_node_types() -> Vec<NodeTypeInfo> {
        crate::platform_registry::canonical_node_types()
    }

    /// Build the list of supported edge types.
    ///
    /// Delegates to `platform_registry::canonical_edge_types()` for single source of truth.
    /// (M-603: Replaced hardcoded edge types list with registry delegation)
    fn build_edge_types() -> Vec<EdgeTypeInfo> {
        crate::platform_registry::canonical_edge_types()
    }

    /// Build the list of built-in templates.
    ///
    /// Delegates to `platform_registry::canonical_templates()` for single source of truth.
    /// (M-603: Replaced hardcoded templates list with registry delegation)
    fn build_templates() -> Vec<TemplateInfo> {
        crate::platform_registry::canonical_templates()
    }

    /// Build the list of available state types.
    ///
    /// Delegates to `platform_registry::canonical_state_types()` for single source of truth.
    /// (M-603: Replaced hardcoded state types list with registry delegation)
    fn build_state_types() -> Vec<StateTypeInfo> {
        crate::platform_registry::canonical_state_types()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_introspection_discover() {
        let platform = PlatformIntrospection::discover();

        // Verify version info
        assert!(!platform.version_info().version.is_empty());
        assert!(!platform.version_info().rust_version.is_empty());

        // Verify features
        assert!(!platform.available_features().is_empty());
        assert!(platform
            .available_features()
            .iter()
            .any(|f| f.name == "checkpointing"));
        assert!(platform
            .available_features()
            .iter()
            .any(|f| f.name == "streaming"));

        // Verify node types
        assert!(!platform.supported_node_types().is_empty());
        assert!(platform
            .supported_node_types()
            .iter()
            .any(|n| n.name == "function"));
        assert!(platform
            .supported_node_types()
            .iter()
            .any(|n| n.name == "agent"));

        // Verify edge types
        assert!(!platform.supported_edge_types().is_empty());
        assert!(platform
            .supported_edge_types()
            .iter()
            .any(|e| e.name == "simple"));
        assert!(platform
            .supported_edge_types()
            .iter()
            .any(|e| e.name == "conditional"));

        // Verify templates
        assert!(!platform.built_in_templates().is_empty());
        assert!(platform
            .built_in_templates()
            .iter()
            .any(|t| t.name == "supervisor"));
        assert!(platform
            .built_in_templates()
            .iter()
            .any(|t| t.name == "react_agent"));

        // Verify state types
        assert!(!platform.state_implementations().is_empty());
    }

    #[test]
    fn test_query_capability_feature() {
        let platform = PlatformIntrospection::discover();

        let cap = platform.query_capability("checkpointing");
        assert!(cap.is_some());
        let cap = cap.unwrap();
        assert_eq!(cap.name, "checkpointing");
        assert_eq!(cap.category, "feature");
        assert!(cap.available);
    }

    #[test]
    fn test_query_capability_node_type() {
        let platform = PlatformIntrospection::discover();

        let cap = platform.query_capability("function");
        assert!(cap.is_some());
        let cap = cap.unwrap();
        assert_eq!(cap.name, "function");
        assert_eq!(cap.category, "node_type");
    }

    #[test]
    fn test_query_capability_edge_type() {
        let platform = PlatformIntrospection::discover();

        // Use "simple" which is unique to edge types
        let cap = platform.query_capability("simple");
        assert!(cap.is_some());
        let cap = cap.unwrap();
        assert_eq!(cap.name, "simple");
        assert_eq!(cap.category, "edge_type");
    }

    #[test]
    fn test_query_capability_template() {
        let platform = PlatformIntrospection::discover();

        let cap = platform.query_capability("supervisor");
        assert!(cap.is_some());
        let cap = cap.unwrap();
        assert_eq!(cap.name, "supervisor");
        assert_eq!(cap.category, "template");
    }

    #[test]
    fn test_query_capability_case_insensitive() {
        let platform = PlatformIntrospection::discover();

        assert!(platform.query_capability("CHECKPOINTING").is_some());
        assert!(platform.query_capability("Checkpointing").is_some());
        assert!(platform.query_capability("checkpointing").is_some());
    }

    #[test]
    fn test_query_capability_not_found() {
        let platform = PlatformIntrospection::discover();

        assert!(platform.query_capability("nonexistent").is_none());
    }

    #[test]
    fn test_to_json() {
        let platform = PlatformIntrospection::discover();
        let json = platform.to_json();

        assert!(json.contains("version"));
        assert!(json.contains("features"));
        assert!(json.contains("node_types"));
        assert!(json.contains("edge_types"));
        assert!(json.contains("templates"));
    }

    #[test]
    fn test_feature_info_builder() {
        let feature = FeatureInfo::simple("test", "Test feature")
            .disabled_by_default()
            .with_opt_out("disable_test()")
            .with_docs("https://example.com/docs");

        assert_eq!(feature.name, "test");
        assert_eq!(feature.description, "Test feature");
        assert!(!feature.default_enabled());
        assert_eq!(feature.opt_out_method(), Some("disable_test()"));
        assert_eq!(
            feature.documentation_url(),
            Some("https://example.com/docs")
        );
    }

    #[test]
    fn test_node_type_info_builder() {
        let node = NodeTypeInfo::new("custom", "Custom node")
            .with_example("graph.add_node(\"custom\", fn);")
            .custom();

        assert_eq!(node.name, "custom");
        assert!(!node.is_builtin);
        assert!(node.example.is_some());
    }

    #[test]
    fn test_edge_type_info_builder() {
        let edge =
            EdgeTypeInfo::new("custom", "Custom edge").with_example("graph.add_custom_edge(a, b);");

        assert_eq!(edge.name, "custom");
        assert!(edge.example.is_some());
    }

    #[test]
    fn test_template_info_builder() {
        let template = TemplateInfo::new("custom", "Custom template")
            .with_use_case("Use case 1")
            .with_use_case("Use case 2")
            .with_example("CustomTemplate::new().build();");

        assert_eq!(template.name, "custom");
        assert_eq!(template.use_cases.len(), 2);
        assert!(template.example.is_some());
    }

    #[test]
    fn test_state_type_info_builder() {
        let state = StateTypeInfo::new("CustomState", "A custom state type").custom();

        assert_eq!(state.name, "CustomState");
        assert!(!state.is_builtin);
    }

    #[test]
    fn test_capability_info_builder() {
        let cap = CapabilityInfo::new("test", "feature", "Test capability").unavailable();

        assert_eq!(cap.name, "test");
        assert_eq!(cap.category, "feature");
        assert!(!cap.available);
    }

    #[test]
    fn test_version_info_default() {
        let version = VersionInfo::default();

        assert!(!version.version.is_empty());
        assert_eq!(version.rust_version, "1.75.0");
    }

    #[test]
    fn test_platform_introspection_default() {
        let platform = PlatformIntrospection::default();

        // Should be equivalent to discover()
        assert!(!platform.version_info().version.is_empty());
        assert!(!platform.available_features().is_empty());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let platform = PlatformIntrospection::discover();
        let json = serde_json::to_string(&platform).unwrap();
        let deserialized: PlatformIntrospection = serde_json::from_str(&json).unwrap();

        assert_eq!(
            platform.version_info().version,
            deserialized.version_info().version
        );
        assert_eq!(
            platform.available_features().len(),
            deserialized.available_features().len()
        );
    }

    #[test]
    fn test_features_have_descriptions() {
        let platform = PlatformIntrospection::discover();

        for feature in platform.available_features() {
            assert!(!feature.name.is_empty(), "Feature name should not be empty");
            assert!(
                !feature.description.is_empty(),
                "Feature {} should have description",
                feature.name
            );
        }
    }

    #[test]
    fn test_node_types_have_descriptions() {
        let platform = PlatformIntrospection::discover();

        for node_type in platform.supported_node_types() {
            assert!(!node_type.name.is_empty());
            assert!(
                !node_type.description.is_empty(),
                "Node type {} should have description",
                node_type.name
            );
        }
    }

    #[test]
    fn test_templates_have_use_cases() {
        let platform = PlatformIntrospection::discover();

        for template in platform.built_in_templates() {
            assert!(!template.name.is_empty());
            assert!(
                !template.use_cases.is_empty(),
                "Template {} should have use cases",
                template.name
            );
        }
    }
}
