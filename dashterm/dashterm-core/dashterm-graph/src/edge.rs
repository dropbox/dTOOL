//! Graph edge definitions

use serde::{Deserialize, Serialize};

/// Type of edge connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeType {
    /// Normal sequential flow
    Normal,
    /// Conditional branch (true path)
    ConditionalTrue,
    /// Conditional branch (false path)
    ConditionalFalse,
    /// Error/exception path
    Error,
    /// Parallel fork
    Fork,
    /// Parallel join
    Join,
    /// Loop back edge
    Loop,
}

impl Default for EdgeType {
    fn default() -> Self {
        EdgeType::Normal
    }
}

/// An edge connecting two nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Edge type for visualization
    pub edge_type: EdgeType,
    /// Optional label
    pub label: Option<String>,
    /// Condition expression (for conditional edges)
    pub condition: Option<String>,
    /// Edge metadata
    pub metadata: EdgeMetadata,
}

impl Default for Edge {
    fn default() -> Self {
        Self {
            edge_type: EdgeType::Normal,
            label: None,
            condition: None,
            metadata: EdgeMetadata::default(),
        }
    }
}

impl Edge {
    pub fn new(edge_type: EdgeType) -> Self {
        Self {
            edge_type,
            ..Default::default()
        }
    }

    pub fn normal() -> Self {
        Self::new(EdgeType::Normal)
    }

    pub fn conditional_true() -> Self {
        Self::new(EdgeType::ConditionalTrue)
    }

    pub fn conditional_false() -> Self {
        Self::new(EdgeType::ConditionalFalse)
    }

    pub fn error() -> Self {
        Self::new(EdgeType::Error)
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }
}

/// Edge metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EdgeMetadata {
    /// Priority for execution ordering
    pub priority: i32,
    /// Whether this edge has been traversed
    pub traversed: bool,
    /// Number of times traversed
    pub traversal_count: u64,
}
