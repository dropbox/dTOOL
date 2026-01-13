// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for registry module
// - clone_on_ref_ptr: Registry uses Arc cloning for shared ownership
#![allow(clippy::clone_on_ref_ptr)]

//! # Graph Registry & Versioning - Multi-Graph Management for AI Agents
//!
//! This module provides registry capabilities for managing multiple graphs and tracking
//! their execution history. It enables AI agents to:
//!
//! - Query "what graphs exist?"
//! - Track execution history
//! - Find graphs by tags or criteria
//! - Monitor running vs completed executions
//!
//! ## GraphRegistry - Central Graph Catalog
//!
//! ```rust,ignore
//! use dashflow::graph_registry::{GraphRegistry, RegistryMetadata};
//!
//! let mut registry = GraphRegistry::new();
//!
//! // Register a graph
//! registry.register(&compiled_graph, RegistryMetadata {
//!     name: "Coding Agent".to_string(),
//!     version: "1.0.0".to_string(),
//!     tags: vec!["coding".to_string(), "production".to_string()],
//!     ..Default::default()
//! });
//!
//! // AI asks: "What graphs are available?"
//! let all_graphs = registry.list_graphs();
//!
//! // AI asks: "Which graphs are for coding?"
//! let coding_graphs = registry.find_by_tag("coding");
//! ```
//!
//! ## ExecutionRegistry - Execution History
//!
//! ```rust,ignore
//! use dashflow::graph_registry::{ExecutionRegistry, ExecutionStatus};
//!
//! let mut exec_registry = ExecutionRegistry::new();
//!
//! // Record execution start
//! exec_registry.record_start("thread_123", "agent_v1");
//!
//! // AI asks: "What executions are running?"
//! let running = exec_registry.list_running();
//!
//! // AI asks: "What executions failed?"
//! let failed = exec_registry.list_by_status(ExecutionStatus::Failed);
//! ```

// Submodules
mod ai_knowledge;
mod execution;
mod state;
mod versioning;

#[cfg(test)]
mod tests;

// Re-exports
pub use ai_knowledge::*;
pub use execution::*;
pub use state::*;
pub use versioning::*;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use crate::introspection::GraphManifest;

// ============================================================================
// GraphRegistry - Central Graph Catalog
// ============================================================================

/// Metadata for a registered graph in the registry
///
/// This extends the basic GraphMetadata with registry-specific fields
/// for catalog management, versioning, and discoverability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryMetadata {
    /// Human-readable name of the graph
    pub name: String,
    /// Semantic version string (e.g., "1.0.0")
    pub version: String,
    /// When this graph was registered
    #[serde(default = "default_system_time")]
    pub created_at: SystemTime,
    /// When this graph was last modified
    #[serde(default = "default_system_time")]
    pub last_modified: SystemTime,
    /// Human-readable description of the graph's purpose
    pub description: String,
    /// Tags for categorization and discovery
    pub tags: Vec<String>,
    /// Optional author/creator information
    pub author: Option<String>,
    /// Custom metadata fields
    #[serde(default)]
    pub custom: HashMap<String, serde_json::Value>,
}

fn default_system_time() -> SystemTime {
    SystemTime::now()
}

impl Default for RegistryMetadata {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: "0.0.0".to_string(),
            created_at: SystemTime::now(),
            last_modified: SystemTime::now(),
            description: String::new(),
            tags: Vec::new(),
            author: None,
            custom: HashMap::new(),
        }
    }
}

impl RegistryMetadata {
    /// Create new registry metadata
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            ..Default::default()
        }
    }

    /// Builder method to set description
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Builder method to add a tag
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Builder method to add multiple tags
    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Builder method to set author
    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Builder method to add custom metadata
    #[must_use]
    pub fn with_custom(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.custom.insert(key.into(), value);
        self
    }

    /// Check if this metadata has a specific tag
    #[must_use]
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Update the last_modified timestamp to now
    pub fn touch(&mut self) {
        self.last_modified = SystemTime::now();
    }
}

/// Entry in the graph registry containing both manifest and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Unique identifier for this graph in the registry
    pub graph_id: String,
    /// The graph's structural manifest
    pub manifest: GraphManifest,
    /// Registry-specific metadata
    pub metadata: RegistryMetadata,
    /// Number of times this graph has been executed
    pub execution_count: u64,
    /// Whether this graph is currently active/enabled
    pub active: bool,
}

impl RegistryEntry {
    /// Create a new registry entry
    #[must_use]
    pub fn new(
        graph_id: impl Into<String>,
        manifest: GraphManifest,
        metadata: RegistryMetadata,
    ) -> Self {
        Self {
            graph_id: graph_id.into(),
            manifest,
            metadata,
            execution_count: 0,
            active: true,
        }
    }

    /// Increment the execution count
    pub fn record_execution(&mut self) {
        self.execution_count += 1;
        self.metadata.touch();
    }

    /// Deactivate this graph
    pub fn deactivate(&mut self) {
        self.active = false;
        self.metadata.touch();
    }

    /// Activate this graph
    pub fn activate(&mut self) {
        self.active = true;
        self.metadata.touch();
    }
}

/// Central registry for all graphs in the system
///
/// The GraphRegistry provides a centralized catalog of all registered graphs,
/// enabling AI agents to discover and query available graphs.
///
/// # Thread Safety
///
/// GraphRegistry uses internal RwLock for thread-safe access.
///
/// # Example
///
/// ```rust
/// use dashflow::graph_registry::{GraphRegistry, RegistryMetadata};
/// use dashflow::introspection::GraphManifest;
///
/// let registry = GraphRegistry::new();
///
/// // Register a graph
/// let manifest = GraphManifest::builder()
///     .entry_point("start")
///     .build()
///     .unwrap();
///
/// registry.register("my_agent", manifest, RegistryMetadata::new("My Agent", "1.0.0"));
///
/// // List all graphs
/// let graphs = registry.list_graphs();
/// assert_eq!(graphs.len(), 1);
///
/// // Find by tag
/// let tagged = registry.find_by_tag("coding");
/// ```
#[derive(Debug)]
pub struct GraphRegistry {
    entries: Arc<RwLock<HashMap<String, RegistryEntry>>>,
}

impl Default for GraphRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for GraphRegistry {
    fn clone(&self) -> Self {
        Self {
            entries: Arc::clone(&self.entries),
        }
    }
}

impl GraphRegistry {
    /// Create a new empty graph registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a graph with its manifest and metadata
    ///
    /// If a graph with the same ID already exists, it will be replaced.
    pub fn register(
        &self,
        graph_id: impl Into<String>,
        manifest: GraphManifest,
        metadata: RegistryMetadata,
    ) {
        let id = graph_id.into();
        let entry = RegistryEntry::new(&id, manifest, metadata);

        let mut entries = self.entries.write().unwrap_or_else(|e| e.into_inner());
        entries.insert(id, entry);
    }

    /// Unregister a graph by ID
    ///
    /// Returns the removed entry if it existed
    pub fn unregister(&self, graph_id: &str) -> Option<RegistryEntry> {
        let mut entries = self.entries.write().unwrap_or_else(|e| e.into_inner());
        entries.remove(graph_id)
    }

    /// Get a graph entry by ID
    #[must_use]
    pub fn get(&self, graph_id: &str) -> Option<RegistryEntry> {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        entries.get(graph_id).cloned()
    }

    /// Check if a graph is registered
    #[must_use]
    pub fn contains(&self, graph_id: &str) -> bool {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        entries.contains_key(graph_id)
    }

    /// List all registered graphs
    #[must_use]
    pub fn list_graphs(&self) -> Vec<RegistryEntry> {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        entries.values().cloned().collect()
    }

    /// List only active graphs
    #[must_use]
    pub fn list_active(&self) -> Vec<RegistryEntry> {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        entries.values().filter(|e| e.active).cloned().collect()
    }

    /// Find graphs by tag
    #[must_use]
    pub fn find_by_tag(&self, tag: &str) -> Vec<RegistryEntry> {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        entries
            .values()
            .filter(|e| e.metadata.has_tag(tag))
            .cloned()
            .collect()
    }

    /// Find graphs by name (case-insensitive substring match)
    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Vec<RegistryEntry> {
        let name_lower = name.to_lowercase();
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        entries
            .values()
            .filter(|e| e.metadata.name.to_lowercase().contains(&name_lower))
            .cloned()
            .collect()
    }

    /// Find graphs by author
    #[must_use]
    pub fn find_by_author(&self, author: &str) -> Vec<RegistryEntry> {
        let author_lower = author.to_lowercase();
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        entries
            .values()
            .filter(|e| {
                e.metadata
                    .author
                    .as_ref()
                    .is_some_and(|a| a.to_lowercase().contains(&author_lower))
            })
            .cloned()
            .collect()
    }

    /// Find graphs by version prefix
    #[must_use]
    pub fn find_by_version_prefix(&self, prefix: &str) -> Vec<RegistryEntry> {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        entries
            .values()
            .filter(|e| e.metadata.version.starts_with(prefix))
            .cloned()
            .collect()
    }

    /// Get total number of registered graphs
    #[must_use]
    pub fn count(&self) -> usize {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        entries.len()
    }

    /// Get graphs sorted by execution count (most executed first)
    #[must_use]
    pub fn most_executed(&self, limit: usize) -> Vec<RegistryEntry> {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        let mut sorted: Vec<_> = entries.values().cloned().collect();
        sorted.sort_by(|a, b| b.execution_count.cmp(&a.execution_count));
        sorted.truncate(limit);
        sorted
    }

    /// Get graphs sorted by last modified (most recent first)
    #[must_use]
    pub fn recently_modified(&self, limit: usize) -> Vec<RegistryEntry> {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        let mut sorted: Vec<_> = entries.values().cloned().collect();
        sorted.sort_by(|a, b| b.metadata.last_modified.cmp(&a.metadata.last_modified));
        sorted.truncate(limit);
        sorted
    }

    /// Update metadata for a graph
    ///
    /// Returns true if the graph was found and updated
    pub fn update_metadata<F>(&self, graph_id: &str, updater: F) -> bool
    where
        F: FnOnce(&mut RegistryMetadata),
    {
        let mut entries = self.entries.write().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = entries.get_mut(graph_id) {
            updater(&mut entry.metadata);
            entry.metadata.touch();
            true
        } else {
            false
        }
    }

    /// Record an execution for a graph
    ///
    /// Returns true if the graph was found and updated
    pub fn record_execution(&self, graph_id: &str) -> bool {
        let mut entries = self.entries.write().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = entries.get_mut(graph_id) {
            entry.record_execution();
            true
        } else {
            false
        }
    }

    /// Deactivate a graph
    ///
    /// Returns true if the graph was found and deactivated
    pub fn deactivate(&self, graph_id: &str) -> bool {
        let mut entries = self.entries.write().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = entries.get_mut(graph_id) {
            entry.deactivate();
            true
        } else {
            false
        }
    }

    /// Activate a graph
    ///
    /// Returns true if the graph was found and activated
    pub fn activate(&self, graph_id: &str) -> bool {
        let mut entries = self.entries.write().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = entries.get_mut(graph_id) {
            entry.activate();
            true
        } else {
            false
        }
    }

    /// Clear all entries from the registry
    pub fn clear(&self) {
        let mut entries = self.entries.write().unwrap_or_else(|e| e.into_inner());
        entries.clear();
    }

    /// Serialize the registry to JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        let entries_vec: Vec<_> = entries.values().collect();
        serde_json::to_string_pretty(&entries_vec)
    }

    /// Get all graph IDs
    #[must_use]
    pub fn graph_ids(&self) -> Vec<String> {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        entries.keys().cloned().collect()
    }
}
