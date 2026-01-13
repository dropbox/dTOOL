// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Graph validation types and utilities
//!
//! This module contains types for representing validation warnings and results
//! discovered during graph compilation.

// ============================================================================
// Graph Validation Types
// ============================================================================

/// Warning types discovered during graph validation
///
/// These represent potential issues that won't prevent compilation
/// but may indicate bugs in graph construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphValidationWarning {
    /// A node exists but cannot be reached from the entry point
    UnreachableNode {
        /// The name of the unreachable node
        node: String,
    },
    /// No path exists from any reachable node to END
    ///
    /// This means the graph may loop indefinitely without terminating.
    NoPathToEnd,
    /// A node has no outgoing edges (dead end)
    ///
    /// This is not necessarily an error (the node may be the last before END),
    /// but can indicate a missing edge.
    DeadEndNode {
        /// The name of the dead-end node
        node: String,
    },
}

impl std::fmt::Display for GraphValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnreachableNode { node } => {
                write!(f, "Node '{}' is unreachable from the entry point", node)
            }
            Self::NoPathToEnd => {
                write!(
                    f,
                    "Graph has no path to END - execution may never terminate"
                )
            }
            Self::DeadEndNode { node } => {
                write!(f, "Node '{}' has no outgoing edges (dead end)", node)
            }
        }
    }
}

/// Result of graph validation
///
/// Contains any warnings discovered during validation. An empty warnings list
/// indicates the graph passed all validation checks.
#[derive(Debug, Clone, Default)]
pub struct GraphValidationResult {
    warnings: Vec<GraphValidationWarning>,
}

impl GraphValidationResult {
    /// Create a new empty validation result
    #[must_use]
    pub fn new() -> Self {
        Self {
            warnings: Vec::new(),
        }
    }

    /// Add a warning to the result
    pub fn add_warning(&mut self, warning: GraphValidationWarning) {
        self.warnings.push(warning);
    }

    /// Check if the graph passed all validations (no warnings)
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.warnings.is_empty()
    }

    /// Get all warnings
    #[must_use]
    pub fn warnings(&self) -> &[GraphValidationWarning] {
        &self.warnings
    }

    /// Get the number of warnings
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    /// Check if there are any unreachable node warnings
    #[must_use]
    pub fn has_unreachable_nodes(&self) -> bool {
        self.warnings
            .iter()
            .any(|w| matches!(w, GraphValidationWarning::UnreachableNode { .. }))
    }

    /// Check if there is a "no path to END" warning
    #[must_use]
    pub fn has_no_path_to_end(&self) -> bool {
        self.warnings
            .iter()
            .any(|w| matches!(w, GraphValidationWarning::NoPathToEnd))
    }

    /// Check if there are any dead-end node warnings
    #[must_use]
    pub fn has_dead_end_nodes(&self) -> bool {
        self.warnings
            .iter()
            .any(|w| matches!(w, GraphValidationWarning::DeadEndNode { .. }))
    }

    /// Get all unreachable node names
    #[must_use]
    pub fn unreachable_nodes(&self) -> Vec<&str> {
        self.warnings
            .iter()
            .filter_map(|w| {
                if let GraphValidationWarning::UnreachableNode { node } = w {
                    Some(node.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all dead-end node names
    #[must_use]
    pub fn dead_end_nodes(&self) -> Vec<&str> {
        self.warnings
            .iter()
            .filter_map(|w| {
                if let GraphValidationWarning::DeadEndNode { node } = w {
                    Some(node.as_str())
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warning_display_strings_are_stable() {
        assert_eq!(
            GraphValidationWarning::UnreachableNode {
                node: "foo".to_string()
            }
            .to_string(),
            "Node 'foo' is unreachable from the entry point"
        );
        assert_eq!(
            GraphValidationWarning::NoPathToEnd.to_string(),
            "Graph has no path to END - execution may never terminate"
        );
        assert_eq!(
            GraphValidationWarning::DeadEndNode {
                node: "bar".to_string()
            }
            .to_string(),
            "Node 'bar' has no outgoing edges (dead end)"
        );
    }

    #[test]
    fn validation_result_tracks_warnings_and_helpers() {
        let mut result = GraphValidationResult::new();
        assert!(result.is_valid());
        assert_eq!(result.warning_count(), 0);
        assert!(result.warnings().is_empty());
        assert!(!result.has_unreachable_nodes());
        assert!(!result.has_no_path_to_end());
        assert!(!result.has_dead_end_nodes());
        assert!(result.unreachable_nodes().is_empty());
        assert!(result.dead_end_nodes().is_empty());

        result.add_warning(GraphValidationWarning::UnreachableNode {
            node: "orphan".to_string(),
        });
        result.add_warning(GraphValidationWarning::NoPathToEnd);
        result.add_warning(GraphValidationWarning::DeadEndNode {
            node: "dead_end".to_string(),
        });

        assert!(!result.is_valid());
        assert_eq!(result.warning_count(), 3);
        assert_eq!(
            result.warnings(),
            &[
                GraphValidationWarning::UnreachableNode {
                    node: "orphan".to_string()
                },
                GraphValidationWarning::NoPathToEnd,
                GraphValidationWarning::DeadEndNode {
                    node: "dead_end".to_string()
                },
            ]
        );

        assert!(result.has_unreachable_nodes());
        assert!(result.has_no_path_to_end());
        assert!(result.has_dead_end_nodes());
        assert_eq!(result.unreachable_nodes(), vec!["orphan"]);
        assert_eq!(result.dead_end_nodes(), vec!["dead_end"]);
    }
}
