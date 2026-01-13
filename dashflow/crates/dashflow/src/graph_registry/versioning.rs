// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Graph Versioning - Version tracking and diffing for graphs
//!
//! This module provides versioning capabilities for graphs:
//! - Structural fingerprinting and change detection
//! - Version history tracking
//! - Diff generation between versions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use crate::introspection::GraphManifest;

// ============================================================================
// Graph Versioning
// ============================================================================

/// Version information for a graph
///
/// GraphVersion captures the structural fingerprint of a graph at a point in time,
/// enabling detection of changes and version comparisons.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphVersion {
    /// Unique identifier for the graph
    pub graph_id: String,
    /// Semantic version string (user-provided)
    pub version: String,
    /// Hash of the graph structure (nodes, edges, configuration)
    pub content_hash: String,
    /// Optional hash of source files (if available)
    pub source_hash: Option<String>,
    /// When this version was created
    pub created_at: SystemTime,
    /// Individual node versions
    pub node_versions: HashMap<String, NodeVersion>,
    /// Number of nodes
    pub node_count: usize,
    /// Number of edges
    pub edge_count: usize,
}

impl GraphVersion {
    /// Create a new graph version from a manifest
    #[must_use]
    pub fn from_manifest(manifest: &GraphManifest, version: impl Into<String>) -> Self {
        let content_hash = Self::compute_content_hash(manifest);
        let node_versions = Self::compute_node_versions(manifest);

        Self {
            graph_id: manifest.graph_id.clone().unwrap_or_default(),
            version: version.into(),
            content_hash,
            source_hash: None,
            created_at: SystemTime::now(),
            node_versions,
            node_count: manifest.node_count(),
            edge_count: manifest.edge_count(),
        }
    }

    /// Compute a hash of the graph structure
    fn compute_content_hash(manifest: &GraphManifest) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash entry point
        manifest.entry_point.hash(&mut hasher);

        // Hash node names (sorted for determinism)
        let mut node_names: Vec<_> = manifest.nodes.keys().collect();
        node_names.sort();
        for name in &node_names {
            name.hash(&mut hasher);
        }

        // Hash edges (sorted for determinism)
        let mut edge_keys: Vec<_> = manifest.edges.keys().collect();
        edge_keys.sort();
        for from in &edge_keys {
            from.hash(&mut hasher);
            if let Some(edges) = manifest.edges.get(*from) {
                for edge in edges {
                    edge.to.hash(&mut hasher);
                    edge.is_conditional.hash(&mut hasher);
                    edge.is_parallel.hash(&mut hasher);
                }
            }
        }

        format!("{:016x}", hasher.finish())
    }

    /// Compute versions for all nodes
    fn compute_node_versions(manifest: &GraphManifest) -> HashMap<String, NodeVersion> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        manifest
            .nodes
            .iter()
            .map(|(name, node_manifest)| {
                let mut hasher = DefaultHasher::new();
                name.hash(&mut hasher);
                node_manifest.description.hash(&mut hasher);
                // Convert node_type to string for hashing (NodeType doesn't implement Hash)
                format!("{:?}", node_manifest.node_type).hash(&mut hasher);

                let node_version = NodeVersion {
                    node_name: name.clone(),
                    version: "1.0.0".to_string(),
                    code_hash: format!("{:016x}", hasher.finish()),
                    source_file: None,
                    source_line: None,
                };
                (name.clone(), node_version)
            })
            .collect()
    }

    /// Set the source hash
    #[must_use]
    pub fn with_source_hash(mut self, hash: impl Into<String>) -> Self {
        self.source_hash = Some(hash.into());
        self
    }

    /// Check if this version differs from another
    #[must_use]
    pub fn has_changed_since(&self, other: &GraphVersion) -> bool {
        self.content_hash != other.content_hash
    }

    /// Compare with another version and return a diff
    #[must_use]
    pub fn diff(&self, other: &GraphVersion) -> GraphDiff {
        let self_nodes: std::collections::HashSet<_> = self.node_versions.keys().collect();
        let other_nodes: std::collections::HashSet<_> = other.node_versions.keys().collect();

        let nodes_added: Vec<String> = self_nodes
            .difference(&other_nodes)
            .map(|s| (*s).clone())
            .collect();

        let nodes_removed: Vec<String> = other_nodes
            .difference(&self_nodes)
            .map(|s| (*s).clone())
            .collect();

        let nodes_modified: Vec<String> = self_nodes
            .intersection(&other_nodes)
            .filter(|name| {
                self.node_versions.get(**name).map(|v| &v.code_hash)
                    != other.node_versions.get(**name).map(|v| &v.code_hash)
            })
            .map(|s| (*s).clone())
            .collect();

        let edges_changed =
            self.edge_count != other.edge_count || self.content_hash != other.content_hash;

        GraphDiff {
            from_version: other.version.clone(),
            to_version: self.version.clone(),
            nodes_added,
            nodes_removed,
            nodes_modified,
            edges_changed,
            content_hash_changed: self.content_hash != other.content_hash,
        }
    }

    /// Generate a human-readable change summary
    #[must_use]
    pub fn change_summary(&self, other: &GraphVersion) -> String {
        let diff = self.diff(other);

        if !diff.has_changes() {
            return "No changes".to_string();
        }

        let mut parts = Vec::new();

        if !diff.nodes_added.is_empty() {
            parts.push(format!("added {} node(s)", diff.nodes_added.len()));
        }
        if !diff.nodes_removed.is_empty() {
            parts.push(format!("removed {} node(s)", diff.nodes_removed.len()));
        }
        if !diff.nodes_modified.is_empty() {
            parts.push(format!("modified {} node(s)", diff.nodes_modified.len()));
        }
        if diff.edges_changed && diff.nodes_added.is_empty() && diff.nodes_removed.is_empty() {
            parts.push("edges changed".to_string());
        }

        format!(
            "Version {} -> {}: {}",
            other.version,
            self.version,
            parts.join(", ")
        )
    }

    /// Serialize to JSON for AI consumption
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Version information for a single node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeVersion {
    /// Name of the node
    pub node_name: String,
    /// Version string
    pub version: String,
    /// Hash of the node's code/structure
    pub code_hash: String,
    /// Source file (if known)
    pub source_file: Option<String>,
    /// Source line (if known)
    pub source_line: Option<usize>,
}

impl NodeVersion {
    /// Create a new node version
    #[must_use]
    pub fn new(
        node_name: impl Into<String>,
        version: impl Into<String>,
        code_hash: impl Into<String>,
    ) -> Self {
        Self {
            node_name: node_name.into(),
            version: version.into(),
            code_hash: code_hash.into(),
            source_file: None,
            source_line: None,
        }
    }

    /// Set source file information
    #[must_use]
    pub fn with_source(mut self, file: impl Into<String>, line: usize) -> Self {
        self.source_file = Some(file.into());
        self.source_line = Some(line);
        self
    }
}

/// Diff between two graph versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDiff {
    /// Version we're comparing from
    pub from_version: String,
    /// Version we're comparing to
    pub to_version: String,
    /// Nodes that were added
    pub nodes_added: Vec<String>,
    /// Nodes that were removed
    pub nodes_removed: Vec<String>,
    /// Nodes that were modified
    pub nodes_modified: Vec<String>,
    /// Whether edges changed
    pub edges_changed: bool,
    /// Whether content hash changed
    pub content_hash_changed: bool,
}

impl GraphDiff {
    /// Check if there are any changes
    #[must_use]
    pub fn has_changes(&self) -> bool {
        !self.nodes_added.is_empty()
            || !self.nodes_removed.is_empty()
            || !self.nodes_modified.is_empty()
            || self.edges_changed
    }

    /// Get total number of node changes
    #[must_use]
    pub fn node_change_count(&self) -> usize {
        self.nodes_added.len() + self.nodes_removed.len() + self.nodes_modified.len()
    }

    /// Generate a detailed report
    #[must_use]
    pub fn detailed_report(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!(
            "Graph Diff: {} -> {}",
            self.from_version, self.to_version
        ));
        lines.push(String::new());

        if !self.nodes_added.is_empty() {
            lines.push("Nodes Added:".to_string());
            for node in &self.nodes_added {
                lines.push(format!("  + {node}"));
            }
            lines.push(String::new());
        }

        if !self.nodes_removed.is_empty() {
            lines.push("Nodes Removed:".to_string());
            for node in &self.nodes_removed {
                lines.push(format!("  - {node}"));
            }
            lines.push(String::new());
        }

        if !self.nodes_modified.is_empty() {
            lines.push("Nodes Modified:".to_string());
            for node in &self.nodes_modified {
                lines.push(format!("  ~ {node}"));
            }
            lines.push(String::new());
        }

        if self.edges_changed {
            lines.push("Edges: Changed".to_string());
        }

        lines.join("\n")
    }

    /// Serialize to JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Version store for tracking graph versions over time
#[derive(Debug)]
pub struct VersionStore {
    versions: Arc<RwLock<HashMap<String, Vec<GraphVersion>>>>,
}

impl Default for VersionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for VersionStore {
    fn clone(&self) -> Self {
        Self {
            versions: Arc::clone(&self.versions),
        }
    }
}

impl VersionStore {
    /// Create a new version store
    #[must_use]
    pub fn new() -> Self {
        Self {
            versions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Save a new version
    pub fn save(&self, version: GraphVersion) {
        let mut versions = self.versions.write().unwrap_or_else(|e| e.into_inner());
        versions
            .entry(version.graph_id.clone())
            .or_default()
            .push(version);
    }

    /// Get the latest version for a graph
    #[must_use]
    pub fn get_latest(&self, graph_id: &str) -> Option<GraphVersion> {
        let versions = self.versions.read().unwrap_or_else(|e| e.into_inner());
        versions.get(graph_id).and_then(|v| v.last()).cloned()
    }

    /// Get a specific version by version string
    #[must_use]
    pub fn get_version(&self, graph_id: &str, version: &str) -> Option<GraphVersion> {
        let versions = self.versions.read().unwrap_or_else(|e| e.into_inner());
        versions
            .get(graph_id)?
            .iter()
            .find(|v| v.version == version)
            .cloned()
    }

    /// Get the previous version (second to last)
    #[must_use]
    pub fn get_previous(&self, graph_id: &str) -> Option<GraphVersion> {
        let versions = self.versions.read().unwrap_or_else(|e| e.into_inner());
        let graph_versions = versions.get(graph_id)?;
        if graph_versions.len() >= 2 {
            Some(graph_versions[graph_versions.len() - 2].clone())
        } else {
            None
        }
    }

    /// List all versions for a graph
    #[must_use]
    pub fn list_versions(&self, graph_id: &str) -> Vec<GraphVersion> {
        let versions = self.versions.read().unwrap_or_else(|e| e.into_inner());
        versions.get(graph_id).cloned().unwrap_or_default()
    }

    /// Get version history (newest first)
    #[must_use]
    pub fn version_history(&self, graph_id: &str, limit: usize) -> Vec<GraphVersion> {
        let versions = self.versions.read().unwrap_or_else(|e| e.into_inner());
        versions
            .get(graph_id)
            .map(|v| {
                let mut reversed = v.to_vec();
                reversed.reverse();
                reversed.truncate(limit);
                reversed
            })
            .unwrap_or_default()
    }

    /// Check if a graph has changed since the last saved version
    #[must_use]
    pub fn has_changed(&self, graph_id: &str, current_hash: &str) -> bool {
        self.get_latest(graph_id)
            .map_or(true, |v| v.content_hash != current_hash)
    }

    /// Get version count for a graph
    #[must_use]
    pub fn version_count(&self, graph_id: &str) -> usize {
        let versions = self.versions.read().unwrap_or_else(|e| e.into_inner());
        versions.get(graph_id).map_or(0, Vec::len)
    }

    /// Get total version count across all graphs
    #[must_use]
    pub fn version_count_total(&self) -> usize {
        let versions = self.versions.read().unwrap_or_else(|e| e.into_inner());
        versions.values().map(Vec::len).sum()
    }

    /// Clear all versions
    pub fn clear(&self) {
        let mut versions = self.versions.write().unwrap_or_else(|e| e.into_inner());
        versions.clear();
    }

    /// Clear versions for a specific graph
    pub fn clear_graph(&self, graph_id: &str) {
        let mut versions = self.versions.write().unwrap_or_else(|e| e.into_inner());
        versions.remove(graph_id);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a test GraphVersion
    fn create_test_version(
        graph_id: &str,
        version: &str,
        content_hash: &str,
        nodes: Vec<(&str, &str)>,
    ) -> GraphVersion {
        let node_versions = nodes
            .into_iter()
            .map(|(name, hash)| {
                (
                    name.to_string(),
                    NodeVersion {
                        node_name: name.to_string(),
                        version: "1.0.0".to_string(),
                        code_hash: hash.to_string(),
                        source_file: None,
                        source_line: None,
                    },
                )
            })
            .collect();

        GraphVersion {
            graph_id: graph_id.to_string(),
            version: version.to_string(),
            content_hash: content_hash.to_string(),
            source_hash: None,
            created_at: SystemTime::now(),
            node_versions,
            node_count: 0,
            edge_count: 0,
        }
    }

    // ========================================================================
    // NodeVersion Tests
    // ========================================================================

    #[test]
    fn test_node_version_new() {
        let nv = NodeVersion::new("test_node", "1.0.0", "abc123");
        assert_eq!(nv.node_name, "test_node");
        assert_eq!(nv.version, "1.0.0");
        assert_eq!(nv.code_hash, "abc123");
        assert!(nv.source_file.is_none());
        assert!(nv.source_line.is_none());
    }

    #[test]
    fn test_node_version_with_source() {
        let nv = NodeVersion::new("test_node", "1.0.0", "abc123").with_source("src/main.rs", 42);
        assert_eq!(nv.source_file, Some("src/main.rs".to_string()));
        assert_eq!(nv.source_line, Some(42));
    }

    #[test]
    fn test_node_version_clone() {
        let nv = NodeVersion::new("node", "1.0.0", "hash").with_source("file.rs", 10);
        let cloned = nv.clone();
        assert_eq!(cloned.node_name, nv.node_name);
        assert_eq!(cloned.version, nv.version);
        assert_eq!(cloned.code_hash, nv.code_hash);
        assert_eq!(cloned.source_file, nv.source_file);
        assert_eq!(cloned.source_line, nv.source_line);
    }

    #[test]
    fn test_node_version_eq() {
        let nv1 = NodeVersion::new("node", "1.0.0", "hash");
        let nv2 = NodeVersion::new("node", "1.0.0", "hash");
        let nv3 = NodeVersion::new("node", "1.0.0", "different");
        assert_eq!(nv1, nv2);
        assert_ne!(nv1, nv3);
    }

    #[test]
    fn test_node_version_serialize() {
        let nv = NodeVersion::new("test", "1.0.0", "abc");
        let json = serde_json::to_string(&nv).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("1.0.0"));
        assert!(json.contains("abc"));
    }

    #[test]
    fn test_node_version_deserialize() {
        let json = r#"{"node_name":"test","version":"2.0.0","code_hash":"xyz"}"#;
        let nv: NodeVersion = serde_json::from_str(json).unwrap();
        assert_eq!(nv.node_name, "test");
        assert_eq!(nv.version, "2.0.0");
        assert_eq!(nv.code_hash, "xyz");
    }

    // ========================================================================
    // GraphVersion Tests
    // ========================================================================

    #[test]
    fn test_graph_version_with_source_hash() {
        let gv = create_test_version("graph1", "1.0.0", "hash1", vec![])
            .with_source_hash("source_abc123");
        assert_eq!(gv.source_hash, Some("source_abc123".to_string()));
    }

    #[test]
    fn test_graph_version_has_changed_since_same() {
        let v1 = create_test_version("g", "1.0", "same_hash", vec![]);
        let v2 = create_test_version("g", "1.1", "same_hash", vec![]);
        assert!(!v1.has_changed_since(&v2));
    }

    #[test]
    fn test_graph_version_has_changed_since_different() {
        let v1 = create_test_version("g", "1.0", "hash_a", vec![]);
        let v2 = create_test_version("g", "1.1", "hash_b", vec![]);
        assert!(v1.has_changed_since(&v2));
    }

    #[test]
    fn test_graph_version_diff_no_changes() {
        let v1 = create_test_version("g", "1.0", "same", vec![("node1", "h1")]);
        let v2 = create_test_version("g", "1.1", "same", vec![("node1", "h1")]);
        let diff = v1.diff(&v2);
        assert!(!diff.has_changes());
        assert!(diff.nodes_added.is_empty());
        assert!(diff.nodes_removed.is_empty());
        assert!(diff.nodes_modified.is_empty());
    }

    #[test]
    fn test_graph_version_diff_nodes_added() {
        let v1 = create_test_version("g", "2.0", "h2", vec![("node1", "h1"), ("node2", "h2")]);
        let v2 = create_test_version("g", "1.0", "h1", vec![("node1", "h1")]);
        let diff = v1.diff(&v2);
        assert!(diff.has_changes());
        assert_eq!(diff.nodes_added.len(), 1);
        assert!(diff.nodes_added.contains(&"node2".to_string()));
    }

    #[test]
    fn test_graph_version_diff_nodes_removed() {
        let v1 = create_test_version("g", "2.0", "h2", vec![("node1", "h1")]);
        let v2 = create_test_version("g", "1.0", "h1", vec![("node1", "h1"), ("node2", "h2")]);
        let diff = v1.diff(&v2);
        assert!(diff.has_changes());
        assert_eq!(diff.nodes_removed.len(), 1);
        assert!(diff.nodes_removed.contains(&"node2".to_string()));
    }

    #[test]
    fn test_graph_version_diff_nodes_modified() {
        let v1 = create_test_version("g", "2.0", "h2", vec![("node1", "new_hash")]);
        let v2 = create_test_version("g", "1.0", "h1", vec![("node1", "old_hash")]);
        let diff = v1.diff(&v2);
        assert!(diff.has_changes());
        assert_eq!(diff.nodes_modified.len(), 1);
        assert!(diff.nodes_modified.contains(&"node1".to_string()));
    }

    #[test]
    fn test_graph_version_diff_edges_changed() {
        let mut v1 = create_test_version("g", "2.0", "h2", vec![]);
        let mut v2 = create_test_version("g", "1.0", "h1", vec![]);
        v1.edge_count = 5;
        v2.edge_count = 3;
        let diff = v1.diff(&v2);
        assert!(diff.edges_changed);
    }

    #[test]
    fn test_graph_version_change_summary_no_changes() {
        let v1 = create_test_version("g", "1.0", "same", vec![("n", "h")]);
        let v2 = create_test_version("g", "1.0", "same", vec![("n", "h")]);
        let summary = v1.change_summary(&v2);
        assert_eq!(summary, "No changes");
    }

    #[test]
    fn test_graph_version_change_summary_with_changes() {
        let v1 = create_test_version("g", "2.0", "h2", vec![("node1", "h1"), ("node2", "h2")]);
        let v2 = create_test_version("g", "1.0", "h1", vec![("node1", "h1")]);
        let summary = v1.change_summary(&v2);
        assert!(summary.contains("added 1 node(s)"));
        assert!(summary.contains("1.0"));
        assert!(summary.contains("2.0"));
    }

    #[test]
    fn test_graph_version_to_json() {
        let gv = create_test_version("test_graph", "1.0.0", "hash123", vec![]);
        let json = gv.to_json().unwrap();
        assert!(json.contains("test_graph"));
        assert!(json.contains("1.0.0"));
        assert!(json.contains("hash123"));
    }

    #[test]
    fn test_graph_version_serialize_deserialize() {
        let original = create_test_version("g", "1.0", "h", vec![("n", "nh")]);
        let json = serde_json::to_string(&original).unwrap();
        let restored: GraphVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.graph_id, original.graph_id);
        assert_eq!(restored.version, original.version);
        assert_eq!(restored.content_hash, original.content_hash);
    }

    // ========================================================================
    // GraphDiff Tests
    // ========================================================================

    #[test]
    fn test_graph_diff_has_changes_empty() {
        let diff = GraphDiff {
            from_version: "1.0".to_string(),
            to_version: "1.1".to_string(),
            nodes_added: vec![],
            nodes_removed: vec![],
            nodes_modified: vec![],
            edges_changed: false,
            content_hash_changed: false,
        };
        assert!(!diff.has_changes());
    }

    #[test]
    fn test_graph_diff_has_changes_added() {
        let diff = GraphDiff {
            from_version: "1.0".to_string(),
            to_version: "1.1".to_string(),
            nodes_added: vec!["node1".to_string()],
            nodes_removed: vec![],
            nodes_modified: vec![],
            edges_changed: false,
            content_hash_changed: true,
        };
        assert!(diff.has_changes());
    }

    #[test]
    fn test_graph_diff_has_changes_removed() {
        let diff = GraphDiff {
            from_version: "1.0".to_string(),
            to_version: "1.1".to_string(),
            nodes_added: vec![],
            nodes_removed: vec!["node1".to_string()],
            nodes_modified: vec![],
            edges_changed: false,
            content_hash_changed: true,
        };
        assert!(diff.has_changes());
    }

    #[test]
    fn test_graph_diff_has_changes_modified() {
        let diff = GraphDiff {
            from_version: "1.0".to_string(),
            to_version: "1.1".to_string(),
            nodes_added: vec![],
            nodes_removed: vec![],
            nodes_modified: vec!["node1".to_string()],
            edges_changed: false,
            content_hash_changed: true,
        };
        assert!(diff.has_changes());
    }

    #[test]
    fn test_graph_diff_has_changes_edges() {
        let diff = GraphDiff {
            from_version: "1.0".to_string(),
            to_version: "1.1".to_string(),
            nodes_added: vec![],
            nodes_removed: vec![],
            nodes_modified: vec![],
            edges_changed: true,
            content_hash_changed: true,
        };
        assert!(diff.has_changes());
    }

    #[test]
    fn test_graph_diff_node_change_count() {
        let diff = GraphDiff {
            from_version: "1.0".to_string(),
            to_version: "1.1".to_string(),
            nodes_added: vec!["a".to_string(), "b".to_string()],
            nodes_removed: vec!["c".to_string()],
            nodes_modified: vec!["d".to_string(), "e".to_string(), "f".to_string()],
            edges_changed: false,
            content_hash_changed: true,
        };
        assert_eq!(diff.node_change_count(), 6);
    }

    #[test]
    fn test_graph_diff_detailed_report_empty() {
        let diff = GraphDiff {
            from_version: "1.0".to_string(),
            to_version: "1.1".to_string(),
            nodes_added: vec![],
            nodes_removed: vec![],
            nodes_modified: vec![],
            edges_changed: false,
            content_hash_changed: false,
        };
        let report = diff.detailed_report();
        assert!(report.contains("Graph Diff: 1.0 -> 1.1"));
    }

    #[test]
    fn test_graph_diff_detailed_report_with_changes() {
        let diff = GraphDiff {
            from_version: "1.0".to_string(),
            to_version: "2.0".to_string(),
            nodes_added: vec!["new_node".to_string()],
            nodes_removed: vec!["old_node".to_string()],
            nodes_modified: vec!["changed_node".to_string()],
            edges_changed: true,
            content_hash_changed: true,
        };
        let report = diff.detailed_report();
        assert!(report.contains("Nodes Added:"));
        assert!(report.contains("+ new_node"));
        assert!(report.contains("Nodes Removed:"));
        assert!(report.contains("- old_node"));
        assert!(report.contains("Nodes Modified:"));
        assert!(report.contains("~ changed_node"));
        assert!(report.contains("Edges: Changed"));
    }

    #[test]
    fn test_graph_diff_to_json() {
        let diff = GraphDiff {
            from_version: "1.0".to_string(),
            to_version: "2.0".to_string(),
            nodes_added: vec!["n".to_string()],
            nodes_removed: vec![],
            nodes_modified: vec![],
            edges_changed: false,
            content_hash_changed: true,
        };
        let json = diff.to_json().unwrap();
        assert!(json.contains("from_version"));
        assert!(json.contains("1.0"));
        assert!(json.contains("2.0"));
    }

    #[test]
    fn test_graph_diff_clone() {
        let diff = GraphDiff {
            from_version: "1.0".to_string(),
            to_version: "2.0".to_string(),
            nodes_added: vec!["a".to_string()],
            nodes_removed: vec!["b".to_string()],
            nodes_modified: vec![],
            edges_changed: true,
            content_hash_changed: true,
        };
        let cloned = diff.clone();
        assert_eq!(cloned.from_version, diff.from_version);
        assert_eq!(cloned.nodes_added, diff.nodes_added);
        assert_eq!(cloned.edges_changed, diff.edges_changed);
    }

    // ========================================================================
    // VersionStore Tests
    // ========================================================================

    #[test]
    fn test_version_store_new() {
        let store = VersionStore::new();
        assert_eq!(store.version_count_total(), 0);
    }

    #[test]
    fn test_version_store_default() {
        let store = VersionStore::default();
        assert_eq!(store.version_count_total(), 0);
    }

    #[test]
    fn test_version_store_save() {
        let store = VersionStore::new();
        let v = create_test_version("graph1", "1.0", "h1", vec![]);
        store.save(v);
        assert_eq!(store.version_count("graph1"), 1);
    }

    #[test]
    fn test_version_store_save_multiple() {
        let store = VersionStore::new();
        store.save(create_test_version("graph1", "1.0", "h1", vec![]));
        store.save(create_test_version("graph1", "2.0", "h2", vec![]));
        store.save(create_test_version("graph2", "1.0", "h3", vec![]));
        assert_eq!(store.version_count("graph1"), 2);
        assert_eq!(store.version_count("graph2"), 1);
        assert_eq!(store.version_count_total(), 3);
    }

    #[test]
    fn test_version_store_get_latest() {
        let store = VersionStore::new();
        store.save(create_test_version("g", "1.0", "h1", vec![]));
        store.save(create_test_version("g", "2.0", "h2", vec![]));

        let latest = store.get_latest("g").unwrap();
        assert_eq!(latest.version, "2.0");
    }

    #[test]
    fn test_version_store_get_latest_not_found() {
        let store = VersionStore::new();
        assert!(store.get_latest("nonexistent").is_none());
    }

    #[test]
    fn test_version_store_get_version() {
        let store = VersionStore::new();
        store.save(create_test_version("g", "1.0", "h1", vec![]));
        store.save(create_test_version("g", "2.0", "h2", vec![]));

        let v = store.get_version("g", "1.0").unwrap();
        assert_eq!(v.version, "1.0");
        assert_eq!(v.content_hash, "h1");
    }

    #[test]
    fn test_version_store_get_version_not_found() {
        let store = VersionStore::new();
        store.save(create_test_version("g", "1.0", "h1", vec![]));
        assert!(store.get_version("g", "999.0").is_none());
    }

    #[test]
    fn test_version_store_get_previous() {
        let store = VersionStore::new();
        store.save(create_test_version("g", "1.0", "h1", vec![]));
        store.save(create_test_version("g", "2.0", "h2", vec![]));
        store.save(create_test_version("g", "3.0", "h3", vec![]));

        let prev = store.get_previous("g").unwrap();
        assert_eq!(prev.version, "2.0");
    }

    #[test]
    fn test_version_store_get_previous_only_one() {
        let store = VersionStore::new();
        store.save(create_test_version("g", "1.0", "h1", vec![]));
        assert!(store.get_previous("g").is_none());
    }

    #[test]
    fn test_version_store_get_previous_none() {
        let store = VersionStore::new();
        assert!(store.get_previous("g").is_none());
    }

    #[test]
    fn test_version_store_list_versions() {
        let store = VersionStore::new();
        store.save(create_test_version("g", "1.0", "h1", vec![]));
        store.save(create_test_version("g", "2.0", "h2", vec![]));

        let versions = store.list_versions("g");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version, "1.0");
        assert_eq!(versions[1].version, "2.0");
    }

    #[test]
    fn test_version_store_list_versions_empty() {
        let store = VersionStore::new();
        let versions = store.list_versions("nonexistent");
        assert!(versions.is_empty());
    }

    #[test]
    fn test_version_store_version_history() {
        let store = VersionStore::new();
        store.save(create_test_version("g", "1.0", "h1", vec![]));
        store.save(create_test_version("g", "2.0", "h2", vec![]));
        store.save(create_test_version("g", "3.0", "h3", vec![]));

        let history = store.version_history("g", 2);
        assert_eq!(history.len(), 2);
        // Should be newest first
        assert_eq!(history[0].version, "3.0");
        assert_eq!(history[1].version, "2.0");
    }

    #[test]
    fn test_version_store_version_history_limit() {
        let store = VersionStore::new();
        for i in 1..=10 {
            store.save(create_test_version(
                "g",
                &format!("{}.0", i),
                &format!("h{}", i),
                vec![],
            ));
        }

        let history = store.version_history("g", 3);
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].version, "10.0");
    }

    #[test]
    fn test_version_store_has_changed_no_versions() {
        let store = VersionStore::new();
        assert!(store.has_changed("g", "any_hash"));
    }

    #[test]
    fn test_version_store_has_changed_same_hash() {
        let store = VersionStore::new();
        store.save(create_test_version("g", "1.0", "same_hash", vec![]));
        assert!(!store.has_changed("g", "same_hash"));
    }

    #[test]
    fn test_version_store_has_changed_different_hash() {
        let store = VersionStore::new();
        store.save(create_test_version("g", "1.0", "old_hash", vec![]));
        assert!(store.has_changed("g", "new_hash"));
    }

    #[test]
    fn test_version_store_clear() {
        let store = VersionStore::new();
        store.save(create_test_version("g1", "1.0", "h1", vec![]));
        store.save(create_test_version("g2", "1.0", "h2", vec![]));
        assert_eq!(store.version_count_total(), 2);

        store.clear();
        assert_eq!(store.version_count_total(), 0);
    }

    #[test]
    fn test_version_store_clear_graph() {
        let store = VersionStore::new();
        store.save(create_test_version("g1", "1.0", "h1", vec![]));
        store.save(create_test_version("g2", "1.0", "h2", vec![]));

        store.clear_graph("g1");
        assert_eq!(store.version_count("g1"), 0);
        assert_eq!(store.version_count("g2"), 1);
    }

    #[test]
    fn test_version_store_clone_shares_state() {
        let store1 = VersionStore::new();
        store1.save(create_test_version("g", "1.0", "h", vec![]));

        let store2 = store1.clone();
        store2.save(create_test_version("g", "2.0", "h2", vec![]));

        // Both should see the new version because they share state
        assert_eq!(store1.version_count("g"), 2);
        assert_eq!(store2.version_count("g"), 2);
    }

    #[test]
    fn test_version_store_thread_safety() {
        use std::thread;

        let store = VersionStore::new();
        let store1 = store.clone();
        let store2 = store.clone();

        let handle1 = thread::spawn(move || {
            for i in 0..100 {
                store1.save(create_test_version(
                    "g1",
                    &format!("{}", i),
                    &format!("h{}", i),
                    vec![],
                ));
            }
        });

        let handle2 = thread::spawn(move || {
            for i in 0..100 {
                store2.save(create_test_version(
                    "g2",
                    &format!("{}", i),
                    &format!("h{}", i),
                    vec![],
                ));
            }
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        assert_eq!(store.version_count("g1"), 100);
        assert_eq!(store.version_count("g2"), 100);
        assert_eq!(store.version_count_total(), 200);
    }
}
