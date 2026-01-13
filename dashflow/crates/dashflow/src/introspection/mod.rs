// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # AI Introspection - Graph Self-Awareness for AI Agents
//!
//! This module provides introspection capabilities that allow AI agents built with DashFlow
//! to understand and query their own structure, state, and execution context.
//!
//! ## Overview
//!
//! AI agents need to understand themselves to make intelligent decisions:
//! - "What graph am I running?"
//! - "What nodes and edges do I have?"
//! - "What's my current state in execution?"
//! - "What capabilities are available to me?"
//!
//! This module provides the foundation for AI self-awareness through structured manifests.
//!
//! ## Graph Manifest Generation
//!
//! The first phase enables AIs to query their graph structure:
//!
//! ```rust,ignore
//! use dashflow::{StateGraph, introspection::GraphManifest};
//!
//! let graph = build_my_agent();
//! let manifest = graph.manifest();
//!
//! // AI can query: "What nodes do I have?"
//! for (name, node) in &manifest.nodes {
//!     println!("Node: {} - {}", name, node.description);
//! }
//!
//! // AI can query: "How are nodes connected?"
//! for (from, edges) in &manifest.edges {
//!     for edge in edges {
//!         println!("Edge: {} -> {}", from, edge.to);
//!     }
//! }
//!
//! // Export as JSON for AI consumption
//! let json = manifest.to_json().unwrap();
//! ```
//!
//! ## Runtime Execution Context
//!
//! The second phase enables AIs to know where they are during execution:
//!
//! ```rust,ignore
//! use dashflow::ExecutionContext;
//!
//! async fn reasoning_node(state: State, context: ExecutionContext) -> Result<State> {
//!     // AI can ask: "How many iterations have I done?"
//!     if context.iteration > 10 {
//!         // Too many iterations, summarize and finish
//!     }
//!
//!     // AI can ask: "What have I already done?"
//!     if context.has_executed("tool_call") {
//!         // Already called tools, don't repeat
//!     }
//!
//!     // AI can ask: "Am I approaching limits?"
//!     if context.is_near_limit() {
//!         // Switch to faster strategy
//!     }
//!
//!     // AI can detect loops
//!     if let Some(looping_node) = context.detect_loop(5) {
//!         // Stuck in a loop, break out
//!     }
//!
//!     Ok(state)
//! }
//! ```

// Module declarations
pub mod bottleneck;
pub mod capability;
pub mod config;
pub mod context;
pub mod decision;
pub mod graph_manifest;
pub mod integration;
pub mod interface;
pub mod module_patterns;
pub mod optimization;
pub mod pattern;
pub mod performance;
pub mod resource;
pub mod state;
pub mod telemetry;
pub mod trace;

// Re-export all public types for backward compatibility
pub use bottleneck::{
    Bottleneck, BottleneckAnalysis, BottleneckBuilder, BottleneckMetric, BottleneckSeverity,
    BottleneckThresholds,
};
pub use capability::{
    CapabilityManifest, CapabilityManifestBuilder, ModelCapability, ModelFeature, StorageBackend,
    StorageFeature, StorageType, ToolManifest, ToolParameter,
};
pub use config::{
    ConfigurationRecommendations, GraphReconfiguration, GraphReconfigurationBuilder,
    RecommendationConfig, ReconfigurationPriority, ReconfigurationType,
};
pub use context::{ExecutionContext, ExecutionContextBuilder};
pub use decision::{DecisionHistory, DecisionLog, DecisionLogBuilder};
pub use graph_manifest::{
    EdgeManifest, FieldSchema, GraphManifest, GraphManifestBuilder, GraphMetadata, NodeConfig,
    NodeManifest, NodeType, StateSchema,
};
pub use interface::{
    InterfaceConfig, IntrospectionInterface, ParsedQuery, QueryResponse, QueryType,
};
pub use optimization::{
    OptimizationAnalysis, OptimizationCategory, OptimizationPriority, OptimizationSuggestion,
    OptimizationSuggestionBuilder,
};
pub use pattern::{
    Pattern, PatternAnalysis, PatternBuilder, PatternCondition, PatternOperator, PatternThresholds,
    PatternType, PatternValue,
};
pub use performance::{
    AlertSeverity, AlertType, PerformanceAlert, PerformanceHistory, PerformanceMetrics,
    PerformanceMetricsBuilder, PerformanceThresholds,
};
pub use resource::{
    BudgetAlert, BudgetAlertSeverity, BudgetAlertType, ResourceUsage, ResourceUsageBuilder,
    ResourceUsageHistory,
};
pub use state::{get_nested_value, json_type_name, StateIntrospection};
pub use telemetry::{OptimizationTrace, TerminationReason, VariantResult};
pub use trace::{ErrorTrace, ExecutionTrace, ExecutionTraceBuilder, NodeExecution};

// Module pattern registry types (Phase 936)
pub use module_patterns::{
    FunctionSignature, LintWarning, ModuleCapabilityEntry, ModulePatternRegistry,
    ReplacementPattern, Severity,
};

#[cfg(test)]
mod tests;
