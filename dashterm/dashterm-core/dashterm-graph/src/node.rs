//! Graph node definitions

use serde::{Deserialize, Serialize};

/// Unique identifier for a node
pub type NodeId = String;

/// Type of computation node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    /// Entry point to the graph
    Start,
    /// Exit point from the graph
    End,
    /// LLM/AI model invocation
    Model,
    /// Tool or function call
    Tool,
    /// Conditional branching
    Condition,
    /// Parallel execution split
    Parallel,
    /// Join from parallel execution
    Join,
    /// Human input/review
    Human,
    /// Custom computation
    Custom,
}

/// Execution status of a node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NodeStatus {
    #[default]
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
    Waiting,
}

/// Group identifier for collapsible node groups
pub type GroupId = String;

/// A node in the computation graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier
    pub id: NodeId,
    /// Display label
    pub label: String,
    /// Node type for visualization
    pub node_type: NodeType,
    /// Current execution status
    pub status: NodeStatus,
    /// Position for layout (x, y) - optional, can be auto-computed
    pub position: Option<(f32, f32)>,
    /// Node metadata
    pub metadata: NodeMetadata,
    /// Execution timing
    pub timing: Option<NodeTiming>,
    /// Group ID if this node belongs to a collapsible group
    pub group_id: Option<GroupId>,
}

/// A group of nodes that can be collapsed/expanded in the visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeGroup {
    /// Unique identifier for the group
    pub id: GroupId,
    /// Display label for the collapsed view
    pub label: String,
    /// Whether the group is currently collapsed
    pub collapsed: bool,
    /// Number of nodes in the group
    pub node_count: usize,
    /// Aggregate status based on child nodes
    pub status: NodeStatus,
    /// Position when collapsed
    pub position: Option<(f32, f32)>,
}

impl Node {
    pub fn new(id: impl Into<String>, label: impl Into<String>, node_type: NodeType) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            node_type,
            status: NodeStatus::Pending,
            position: None,
            metadata: NodeMetadata::default(),
            timing: None,
            group_id: None,
        }
    }

    /// Create a node that belongs to a group
    pub fn with_group(mut self, group_id: impl Into<String>) -> Self {
        self.group_id = Some(group_id.into());
        self
    }

    pub fn start(id: impl Into<String>) -> Self {
        Self::new(id, "Start", NodeType::Start)
    }

    pub fn end(id: impl Into<String>) -> Self {
        Self::new(id, "End", NodeType::End)
    }

    pub fn model(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, label, NodeType::Model)
    }

    pub fn tool(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, label, NodeType::Tool)
    }

    pub fn condition(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, label, NodeType::Condition)
    }

    pub fn with_position(mut self, x: f32, y: f32) -> Self {
        self.position = Some((x, y));
        self
    }
}

/// Node metadata for display and debugging
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Description of what this node does
    pub description: Option<String>,
    /// Input schema (JSON Schema string)
    pub input_schema: Option<String>,
    /// Output schema (JSON Schema string)
    pub output_schema: Option<String>,
    /// Tags for filtering/grouping
    pub tags: Vec<String>,
    /// Custom key-value properties
    pub properties: std::collections::HashMap<String, String>,
}

/// Execution timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTiming {
    /// When execution started (ms since epoch)
    pub started_at: u64,
    /// When execution completed (ms since epoch)
    pub completed_at: Option<u64>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
}

impl NodeGroup {
    /// Create a new node group
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            collapsed: false,
            node_count: 0,
            status: NodeStatus::Pending,
            position: None,
        }
    }

    /// Create a group for tool operations (e.g., "Tools: Read, Edit, Write")
    pub fn tool_group(id: impl Into<String>, tool_names: &[&str]) -> Self {
        let label = if tool_names.len() <= 3 {
            format!("Tools: {}", tool_names.join(", "))
        } else {
            format!("Tools ({} operations)", tool_names.len())
        };
        Self {
            id: id.into(),
            label,
            collapsed: true,
            node_count: tool_names.len(),
            status: NodeStatus::Pending,
            position: None,
        }
    }

    /// Update status based on child node statuses
    pub fn update_status_from_children(&mut self, statuses: &[NodeStatus]) {
        self.node_count = statuses.len();
        if statuses.is_empty() {
            self.status = NodeStatus::Pending;
            return;
        }

        // If any failed, group is failed
        if statuses.iter().any(|s| matches!(s, NodeStatus::Failed)) {
            self.status = NodeStatus::Failed;
        }
        // If any running, group is running
        else if statuses.iter().any(|s| matches!(s, NodeStatus::Running)) {
            self.status = NodeStatus::Running;
        }
        // If all success, group is success
        else if statuses.iter().all(|s| matches!(s, NodeStatus::Success)) {
            self.status = NodeStatus::Success;
        }
        // If all skipped, group is skipped
        else if statuses.iter().all(|s| matches!(s, NodeStatus::Skipped)) {
            self.status = NodeStatus::Skipped;
        }
        // Mixed pending/waiting
        else {
            self.status = NodeStatus::Waiting;
        }
    }
}

impl NodeTiming {
    pub fn start_now() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        Self {
            started_at,
            completed_at: None,
            duration_ms: None,
        }
    }

    pub fn complete(&mut self) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let completed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.completed_at = Some(completed_at);
        self.duration_ms = Some(completed_at - self.started_at);
    }
}
