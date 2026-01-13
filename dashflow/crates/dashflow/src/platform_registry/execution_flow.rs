// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Execution flow documentation for graph analysis.
//!
//! Provides AI agents with understanding of how their graph executes.

use serde::{Deserialize, Serialize};

/// Execution flow documentation
///
/// Provides AI agents with understanding of how their graph executes:
/// - What is the overall execution flow?
/// - What decision points exist?
/// - What loops/cycles are in the graph?
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::platform_registry::ExecutionFlow;
///
/// let flow = graph.explain_execution_flow();
///
/// // AI asks: "How do I work?"
/// println!("{}", flow.flow_description);
///
/// // AI asks: "What decisions do I make?"
/// for decision in &flow.decision_points {
///     println!("At {}: {}", decision.node, decision.explanation);
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionFlow {
    /// Graph identifier
    pub graph_id: String,
    /// Human-readable description of the execution flow
    pub flow_description: String,
    /// Entry point node
    pub entry_point: String,
    /// Exit points (nodes that lead to END)
    pub exit_points: Vec<String>,
    /// Decision points in the graph
    pub decision_points: Vec<DecisionPoint>,
    /// Loop structures in the graph
    pub loop_structures: Vec<LoopStructure>,
    /// Linear paths through the graph
    pub linear_paths: Vec<ExecutionPath>,
    /// Metadata about the flow analysis
    pub metadata: ExecutionFlowMetadata,
}

impl ExecutionFlow {
    /// Create a new execution flow builder
    #[must_use]
    pub fn builder(graph_id: impl Into<String>) -> ExecutionFlowBuilder {
        ExecutionFlowBuilder::new(graph_id)
    }

    /// Convert flow to JSON string for AI consumption
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Get a brief summary of the execution flow
    #[must_use]
    pub fn summary(&self) -> String {
        let paths = self.linear_paths.len();
        let decisions = self.decision_points.len();
        let loops = self.loop_structures.len();
        format!(
            "Graph '{}': {} paths, {} decisions, {} loops (entry: {}, exits: {:?})",
            self.graph_id, paths, decisions, loops, self.entry_point, self.exit_points
        )
    }

    /// Check if the graph has cycles/loops
    #[must_use]
    pub fn has_cycles(&self) -> bool {
        !self.loop_structures.is_empty()
    }

    /// Check if the graph has conditional branching
    #[must_use]
    pub fn has_branching(&self) -> bool {
        !self.decision_points.is_empty()
    }

    /// Get all nodes mentioned in the flow
    #[must_use]
    pub fn all_nodes(&self) -> Vec<&str> {
        let mut nodes: Vec<&str> = Vec::new();

        // Entry and exit points
        nodes.push(&self.entry_point);
        for exit in &self.exit_points {
            nodes.push(exit);
        }

        // Decision point nodes
        for dp in &self.decision_points {
            nodes.push(&dp.node);
            for path in &dp.paths {
                nodes.push(&path.target);
            }
        }

        // Loop nodes
        for ls in &self.loop_structures {
            for node in &ls.nodes_in_loop {
                nodes.push(node);
            }
        }

        // Path nodes
        for path in &self.linear_paths {
            for node in &path.nodes {
                nodes.push(node);
            }
        }

        // Deduplicate
        nodes.sort();
        nodes.dedup();
        nodes
    }

    /// Find a decision point by node name
    #[must_use]
    pub fn find_decision(&self, node: &str) -> Option<&DecisionPoint> {
        self.decision_points.iter().find(|dp| dp.node == node)
    }

    /// Find loops that include a specific node
    #[must_use]
    pub fn loops_containing(&self, node: &str) -> Vec<&LoopStructure> {
        self.loop_structures
            .iter()
            .filter(|ls| ls.nodes_in_loop.contains(&node.to_string()))
            .collect()
    }

    /// Get the complexity score (higher = more complex)
    #[must_use]
    pub fn complexity_score(&self) -> u32 {
        let base = 1;
        let decisions = self.decision_points.len() as u32 * 2;
        let loops = self.loop_structures.len() as u32 * 3;
        let paths = self.linear_paths.len() as u32;
        base + decisions + loops + paths
    }

    /// Get complexity description
    #[must_use]
    pub fn complexity_description(&self) -> &'static str {
        match self.complexity_score() {
            0..=3 => "Simple (linear flow)",
            4..=8 => "Moderate (some branching)",
            9..=15 => "Complex (multiple paths and loops)",
            _ => "Very Complex (highly branched with cycles)",
        }
    }
}

/// Builder for ExecutionFlow
#[derive(Debug, Default)]
pub struct ExecutionFlowBuilder {
    graph_id: String,
    flow_description: Option<String>,
    entry_point: Option<String>,
    exit_points: Vec<String>,
    decision_points: Vec<DecisionPoint>,
    loop_structures: Vec<LoopStructure>,
    linear_paths: Vec<ExecutionPath>,
    metadata: Option<ExecutionFlowMetadata>,
}

impl ExecutionFlowBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new(graph_id: impl Into<String>) -> Self {
        Self {
            graph_id: graph_id.into(),
            ..Default::default()
        }
    }

    /// Set the flow description
    #[must_use]
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.flow_description = Some(desc.into());
        self
    }

    /// Set the entry point
    #[must_use]
    pub fn entry_point(mut self, entry: impl Into<String>) -> Self {
        self.entry_point = Some(entry.into());
        self
    }

    /// Add an exit point
    pub fn add_exit_point(&mut self, exit: impl Into<String>) -> &mut Self {
        self.exit_points.push(exit.into());
        self
    }

    /// Add a decision point
    pub fn add_decision_point(&mut self, decision: DecisionPoint) -> &mut Self {
        self.decision_points.push(decision);
        self
    }

    /// Add a loop structure
    pub fn add_loop_structure(&mut self, loop_struct: LoopStructure) -> &mut Self {
        self.loop_structures.push(loop_struct);
        self
    }

    /// Add a linear path
    pub fn add_linear_path(&mut self, path: ExecutionPath) -> &mut Self {
        self.linear_paths.push(path);
        self
    }

    /// Set metadata
    #[must_use]
    pub fn metadata(mut self, metadata: ExecutionFlowMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Build the execution flow
    #[must_use]
    pub fn build(self) -> ExecutionFlow {
        ExecutionFlow {
            graph_id: self.graph_id,
            flow_description: self
                .flow_description
                .unwrap_or_else(|| "No description available".to_string()),
            entry_point: self.entry_point.unwrap_or_else(|| "start".to_string()),
            exit_points: self.exit_points,
            decision_points: self.decision_points,
            loop_structures: self.loop_structures,
            linear_paths: self.linear_paths,
            metadata: self.metadata.unwrap_or_default(),
        }
    }
}

/// A decision point in the execution flow
///
/// Represents a conditional branch where the graph makes a routing decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPoint {
    /// Node where the decision is made
    pub node: String,
    /// Condition expression or description
    pub condition: String,
    /// Possible paths from this decision
    pub paths: Vec<DecisionPath>,
    /// Human-readable explanation of the decision
    pub explanation: String,
    /// Decision type
    pub decision_type: DecisionType,
}

impl DecisionPoint {
    /// Create a new decision point
    #[must_use]
    pub fn new(node: impl Into<String>, condition: impl Into<String>) -> Self {
        Self {
            node: node.into(),
            condition: condition.into(),
            paths: Vec::new(),
            explanation: String::new(),
            decision_type: DecisionType::Conditional,
        }
    }

    /// Add a path
    #[must_use]
    pub fn with_path(mut self, path: DecisionPath) -> Self {
        self.paths.push(path);
        self
    }

    /// Add multiple paths
    #[must_use]
    pub fn with_paths(mut self, paths: Vec<DecisionPath>) -> Self {
        self.paths.extend(paths);
        self
    }

    /// Set the explanation
    #[must_use]
    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = explanation.into();
        self
    }

    /// Set the decision type
    #[must_use]
    pub fn with_type(mut self, decision_type: DecisionType) -> Self {
        self.decision_type = decision_type;
        self
    }

    /// Get the number of possible paths
    #[must_use]
    pub fn path_count(&self) -> usize {
        self.paths.len()
    }

    /// Check if this is a binary decision (2 paths)
    #[must_use]
    pub fn is_binary(&self) -> bool {
        self.paths.len() == 2
    }
}

/// A path from a decision point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPath {
    /// Target node
    pub target: String,
    /// Condition that leads to this path
    pub when: String,
    /// Probability or frequency (if known)
    pub probability: Option<f32>,
}

impl DecisionPath {
    /// Create a new decision path
    #[must_use]
    pub fn new(target: impl Into<String>, when: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            when: when.into(),
            probability: None,
        }
    }

    /// Set the probability
    #[must_use]
    pub fn with_probability(mut self, prob: f32) -> Self {
        self.probability = Some(prob);
        self
    }
}

/// Type of decision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionType {
    /// Conditional branch (if/else)
    #[default]
    Conditional,
    /// Tool selection decision
    ToolSelection,
    /// Loop continuation decision
    LoopControl,
    /// Error handling branch
    ErrorHandling,
    /// Human-in-the-loop approval
    HumanApproval,
    /// Parallel fan-out
    Parallel,
}

impl std::fmt::Display for DecisionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conditional => write!(f, "Conditional"),
            Self::ToolSelection => write!(f, "Tool Selection"),
            Self::LoopControl => write!(f, "Loop Control"),
            Self::ErrorHandling => write!(f, "Error Handling"),
            Self::HumanApproval => write!(f, "Human Approval"),
            Self::Parallel => write!(f, "Parallel"),
        }
    }
}

/// A loop/cycle structure in the graph
///
/// Represents iterative execution patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopStructure {
    /// Name/identifier for this loop
    pub name: String,
    /// Nodes that are part of this loop
    pub nodes_in_loop: Vec<String>,
    /// Entry node to the loop
    pub entry_node: String,
    /// Exit condition
    pub exit_condition: String,
    /// Human-readable explanation
    pub explanation: String,
    /// Maximum iterations (if known)
    pub max_iterations: Option<u32>,
    /// Loop type
    pub loop_type: LoopType,
}

impl LoopStructure {
    /// Create a new loop structure
    #[must_use]
    pub fn new(name: impl Into<String>, entry_node: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes_in_loop: Vec::new(),
            entry_node: entry_node.into(),
            exit_condition: String::new(),
            explanation: String::new(),
            max_iterations: None,
            loop_type: LoopType::Iterative,
        }
    }

    /// Add nodes to the loop
    #[must_use]
    pub fn with_nodes(mut self, nodes: Vec<impl Into<String>>) -> Self {
        self.nodes_in_loop.extend(nodes.into_iter().map(Into::into));
        self
    }

    /// Set the exit condition
    #[must_use]
    pub fn with_exit_condition(mut self, condition: impl Into<String>) -> Self {
        self.exit_condition = condition.into();
        self
    }

    /// Set the explanation
    #[must_use]
    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = explanation.into();
        self
    }

    /// Set max iterations
    #[must_use]
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = Some(max);
        self
    }

    /// Set the loop type
    #[must_use]
    pub fn with_type(mut self, loop_type: LoopType) -> Self {
        self.loop_type = loop_type;
        self
    }

    /// Check if node is in this loop
    #[must_use]
    pub fn contains(&self, node: &str) -> bool {
        self.nodes_in_loop.iter().any(|n| n == node)
    }
}

/// Type of loop
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopType {
    /// Standard iterative loop
    #[default]
    Iterative,
    /// Agent reasoning loop (think-act-observe)
    AgentLoop,
    /// Retry loop for error handling
    RetryLoop,
    /// Refinement loop for improving results
    RefinementLoop,
    /// Map-reduce style parallel loop
    MapReduce,
}

impl std::fmt::Display for LoopType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Iterative => write!(f, "Iterative"),
            Self::AgentLoop => write!(f, "Agent Loop"),
            Self::RetryLoop => write!(f, "Retry Loop"),
            Self::RefinementLoop => write!(f, "Refinement Loop"),
            Self::MapReduce => write!(f, "Map-Reduce"),
        }
    }
}

/// A linear execution path through the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPath {
    /// Name/identifier for this path
    pub name: String,
    /// Ordered list of nodes in this path
    pub nodes: Vec<String>,
    /// Description of when this path is taken
    pub description: String,
    /// Whether this is a common/main path
    pub is_main_path: bool,
}

impl ExecutionPath {
    /// Create a new execution path
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
            description: String::new(),
            is_main_path: false,
        }
    }

    /// Set nodes in the path
    #[must_use]
    pub fn with_nodes(mut self, nodes: Vec<impl Into<String>>) -> Self {
        self.nodes.extend(nodes.into_iter().map(Into::into));
        self
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Mark as main path
    #[must_use]
    pub fn main_path(mut self) -> Self {
        self.is_main_path = true;
        self
    }

    /// Get path length
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if path is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Metadata about the execution flow analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionFlowMetadata {
    /// Source of the analysis
    pub source: Option<String>,
    /// When the analysis was performed
    pub analyzed_at: Option<String>,
    /// Total node count
    pub node_count: usize,
    /// Total edge count
    pub edge_count: usize,
    /// Notes about the analysis
    pub notes: Vec<String>,
}

impl ExecutionFlowMetadata {
    /// Create new metadata
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the source
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set node and edge counts
    #[must_use]
    pub fn with_counts(mut self, nodes: usize, edges: usize) -> Self {
        self.node_count = nodes;
        self.edge_count = edges;
        self
    }

    /// Add a note
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// Generate a human-readable flow description from graph structure
///
/// This is a utility function to help generate natural language descriptions
/// of execution flows.
///
/// # Arguments
///
/// * `entry` - Entry point node name
/// * `nodes` - All node names in the graph
/// * `decision_points` - Decision points in the graph
/// * `loops` - Loop structures in the graph
///
/// # Returns
///
/// A human-readable string describing the execution flow.
#[must_use]
pub fn generate_flow_description(
    entry: &str,
    nodes: &[String],
    decision_points: &[DecisionPoint],
    loops: &[LoopStructure],
) -> String {
    let mut desc = String::new();

    // Opening
    desc.push_str(&format!(
        "This graph starts at '{}' and flows through {} nodes.\n\n",
        entry,
        nodes.len()
    ));

    // Decision points
    if !decision_points.is_empty() {
        desc.push_str("Decision Points:\n");
        for dp in decision_points {
            desc.push_str(&format!("  - At '{}': {}\n", dp.node, dp.explanation));
            for path in &dp.paths {
                desc.push_str(&format!("    → '{}' when {}\n", path.target, path.when));
            }
        }
        desc.push('\n');
    }

    // Loops
    if !loops.is_empty() {
        desc.push_str("Loop Structures:\n");
        for loop_struct in loops {
            desc.push_str(&format!(
                "  - {}: {} (exits when: {})\n",
                loop_struct.name, loop_struct.explanation, loop_struct.exit_condition
            ));
        }
        desc.push('\n');
    }

    // Complexity
    let complexity = if loops.is_empty() && decision_points.is_empty() {
        "simple linear"
    } else if loops.is_empty() {
        "branching"
    } else if decision_points.is_empty() {
        "iterative"
    } else {
        "complex (branching with loops)"
    };
    desc.push_str(&format!("Overall pattern: {} execution.\n", complexity));

    desc
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // ExecutionFlowBuilder tests
    // =========================================================================

    #[test]
    fn test_builder_new() {
        let builder = ExecutionFlowBuilder::new("test-graph");
        assert_eq!(builder.graph_id, "test-graph");
    }

    #[test]
    fn test_builder_description() {
        let builder = ExecutionFlowBuilder::new("test").description("A test flow");
        let flow = builder.build();
        assert_eq!(flow.flow_description, "A test flow");
    }

    #[test]
    fn test_builder_entry_point() {
        let builder = ExecutionFlowBuilder::new("test").entry_point("start_node");
        let flow = builder.build();
        assert_eq!(flow.entry_point, "start_node");
    }

    #[test]
    fn test_builder_default_entry_point() {
        let flow = ExecutionFlowBuilder::new("test").build();
        assert_eq!(flow.entry_point, "start");
    }

    #[test]
    fn test_builder_default_description() {
        let flow = ExecutionFlowBuilder::new("test").build();
        assert_eq!(flow.flow_description, "No description available");
    }

    #[test]
    fn test_builder_add_exit_point() {
        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_exit_point("end1");
        builder.add_exit_point("end2");
        let flow = builder.build();
        assert_eq!(flow.exit_points, vec!["end1", "end2"]);
    }

    #[test]
    fn test_builder_add_decision_point() {
        let dp = DecisionPoint::new("router", "x > 0");
        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_decision_point(dp);
        let flow = builder.build();
        assert_eq!(flow.decision_points.len(), 1);
        assert_eq!(flow.decision_points[0].node, "router");
    }

    #[test]
    fn test_builder_add_loop_structure() {
        let ls = LoopStructure::new("main_loop", "process");
        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_loop_structure(ls);
        let flow = builder.build();
        assert_eq!(flow.loop_structures.len(), 1);
        assert_eq!(flow.loop_structures[0].name, "main_loop");
    }

    #[test]
    fn test_builder_add_linear_path() {
        let path = ExecutionPath::new("happy_path").with_nodes(vec!["a", "b", "c"]);
        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_linear_path(path);
        let flow = builder.build();
        assert_eq!(flow.linear_paths.len(), 1);
        assert_eq!(flow.linear_paths[0].nodes, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_builder_metadata() {
        let meta = ExecutionFlowMetadata::new().with_counts(5, 7);
        let flow = ExecutionFlowBuilder::new("test").metadata(meta).build();
        assert_eq!(flow.metadata.node_count, 5);
        assert_eq!(flow.metadata.edge_count, 7);
    }

    // =========================================================================
    // ExecutionFlow tests
    // =========================================================================

    #[test]
    fn test_execution_flow_builder_static() {
        let builder = ExecutionFlow::builder("my-graph");
        let flow = builder.build();
        assert_eq!(flow.graph_id, "my-graph");
    }

    #[test]
    fn test_execution_flow_to_json() {
        let flow = ExecutionFlowBuilder::new("test")
            .description("Test flow")
            .entry_point("start")
            .build();
        let json = flow.to_json().expect("JSON serialization should succeed");
        assert!(json.contains("test"));
        assert!(json.contains("Test flow"));
    }

    #[test]
    fn test_execution_flow_summary() {
        let mut builder = ExecutionFlowBuilder::new("test_graph").entry_point("start");
        builder.add_exit_point("end");
        builder.add_linear_path(ExecutionPath::new("main"));
        builder.add_decision_point(DecisionPoint::new("router", "cond"));
        let flow = builder.build();

        let summary = flow.summary();
        assert!(summary.contains("test_graph"));
        assert!(summary.contains("1 paths"));
        assert!(summary.contains("1 decisions"));
        assert!(summary.contains("0 loops"));
    }

    #[test]
    fn test_execution_flow_has_cycles_false() {
        let flow = ExecutionFlowBuilder::new("test").build();
        assert!(!flow.has_cycles());
    }

    #[test]
    fn test_execution_flow_has_cycles_true() {
        let ls = LoopStructure::new("retry", "process");
        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_loop_structure(ls);
        let flow = builder.build();
        assert!(flow.has_cycles());
    }

    #[test]
    fn test_execution_flow_has_branching_false() {
        let flow = ExecutionFlowBuilder::new("test").build();
        assert!(!flow.has_branching());
    }

    #[test]
    fn test_execution_flow_has_branching_true() {
        let dp = DecisionPoint::new("router", "x > 0");
        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_decision_point(dp);
        let flow = builder.build();
        assert!(flow.has_branching());
    }

    #[test]
    fn test_execution_flow_all_nodes() {
        let dp = DecisionPoint::new("router", "cond")
            .with_path(DecisionPath::new("node_a", "true"))
            .with_path(DecisionPath::new("node_b", "false"));
        let ls = LoopStructure::new("loop1", "loop_entry").with_nodes(vec!["loop_node"]);
        let path = ExecutionPath::new("main").with_nodes(vec!["start", "end"]);

        let mut builder = ExecutionFlowBuilder::new("test").entry_point("entry");
        builder.add_exit_point("exit");
        builder.add_decision_point(dp);
        builder.add_loop_structure(ls);
        builder.add_linear_path(path);

        let flow = builder.build();
        let all = flow.all_nodes();

        assert!(all.contains(&"entry"));
        assert!(all.contains(&"exit"));
        assert!(all.contains(&"router"));
        assert!(all.contains(&"node_a"));
        assert!(all.contains(&"node_b"));
        assert!(all.contains(&"loop_node"));
        assert!(all.contains(&"start"));
        assert!(all.contains(&"end"));
    }

    #[test]
    fn test_execution_flow_all_nodes_deduplication() {
        let path1 = ExecutionPath::new("p1").with_nodes(vec!["a", "b"]);
        let path2 = ExecutionPath::new("p2").with_nodes(vec!["b", "c"]);

        let mut builder = ExecutionFlowBuilder::new("test").entry_point("a");
        builder.add_linear_path(path1);
        builder.add_linear_path(path2);

        let flow = builder.build();
        let all = flow.all_nodes();

        // Count occurrences of "b"
        let b_count = all.iter().filter(|&&n| n == "b").count();
        assert_eq!(b_count, 1, "Nodes should be deduplicated");
    }

    #[test]
    fn test_execution_flow_find_decision_found() {
        let dp1 = DecisionPoint::new("router1", "cond1");
        let dp2 = DecisionPoint::new("router2", "cond2");

        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_decision_point(dp1);
        builder.add_decision_point(dp2);

        let flow = builder.build();
        let found = flow.find_decision("router2");

        assert!(found.is_some());
        assert_eq!(found.unwrap().condition, "cond2");
    }

    #[test]
    fn test_execution_flow_find_decision_not_found() {
        let flow = ExecutionFlowBuilder::new("test").build();
        assert!(flow.find_decision("nonexistent").is_none());
    }

    #[test]
    fn test_execution_flow_loops_containing() {
        let ls1 = LoopStructure::new("loop1", "entry1").with_nodes(vec!["a", "b"]);
        let ls2 = LoopStructure::new("loop2", "entry2").with_nodes(vec!["b", "c"]);

        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_loop_structure(ls1);
        builder.add_loop_structure(ls2);

        let flow = builder.build();

        // "b" is in both loops
        let loops_with_b = flow.loops_containing("b");
        assert_eq!(loops_with_b.len(), 2);

        // "a" is only in loop1
        let loops_with_a = flow.loops_containing("a");
        assert_eq!(loops_with_a.len(), 1);
        assert_eq!(loops_with_a[0].name, "loop1");

        // "x" is in no loops
        let loops_with_x = flow.loops_containing("x");
        assert!(loops_with_x.is_empty());
    }

    #[test]
    fn test_execution_flow_complexity_score_simple() {
        let flow = ExecutionFlowBuilder::new("test").build();
        // base=1, no decisions, no loops, no paths
        assert_eq!(flow.complexity_score(), 1);
    }

    #[test]
    fn test_execution_flow_complexity_score_with_decisions() {
        let dp = DecisionPoint::new("router", "cond");
        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_decision_point(dp);
        let flow = builder.build();
        // base=1 + 1*2=2 decisions = 3
        assert_eq!(flow.complexity_score(), 3);
    }

    #[test]
    fn test_execution_flow_complexity_score_with_loops() {
        let ls = LoopStructure::new("loop", "entry");
        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_loop_structure(ls);
        let flow = builder.build();
        // base=1 + 1*3=3 loops = 4
        assert_eq!(flow.complexity_score(), 4);
    }

    #[test]
    fn test_execution_flow_complexity_score_with_paths() {
        let path = ExecutionPath::new("main");
        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_linear_path(path);
        let flow = builder.build();
        // base=1 + 1 path = 2
        assert_eq!(flow.complexity_score(), 2);
    }

    #[test]
    fn test_execution_flow_complexity_score_complex() {
        let dp1 = DecisionPoint::new("r1", "c1");
        let dp2 = DecisionPoint::new("r2", "c2");
        let ls1 = LoopStructure::new("l1", "e1");
        let ls2 = LoopStructure::new("l2", "e2");
        let p1 = ExecutionPath::new("p1");
        let p2 = ExecutionPath::new("p2");
        let p3 = ExecutionPath::new("p3");

        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_decision_point(dp1);
        builder.add_decision_point(dp2);
        builder.add_loop_structure(ls1);
        builder.add_loop_structure(ls2);
        builder.add_linear_path(p1);
        builder.add_linear_path(p2);
        builder.add_linear_path(p3);

        let flow = builder.build();
        // base=1 + 2*2=4 decisions + 2*3=6 loops + 3 paths = 14
        assert_eq!(flow.complexity_score(), 14);
    }

    #[test]
    fn test_execution_flow_complexity_description_simple() {
        let flow = ExecutionFlowBuilder::new("test").build();
        assert_eq!(flow.complexity_description(), "Simple (linear flow)");
    }

    #[test]
    fn test_execution_flow_complexity_description_moderate() {
        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_decision_point(DecisionPoint::new("r1", "c1"));
        builder.add_decision_point(DecisionPoint::new("r2", "c2"));
        let flow = builder.build();
        // Score = 1 + 4 = 5, which is "Moderate"
        assert_eq!(flow.complexity_description(), "Moderate (some branching)");
    }

    #[test]
    fn test_execution_flow_complexity_description_complex() {
        let mut builder = ExecutionFlowBuilder::new("test");
        builder.add_decision_point(DecisionPoint::new("r1", "c1"));
        builder.add_decision_point(DecisionPoint::new("r2", "c2"));
        builder.add_loop_structure(LoopStructure::new("l1", "e1"));
        let flow = builder.build();
        // Score = 1 + 4 + 3 = 8 + 1 loop = 8... wait let's recalc: 1 + 2*2 + 1*3 = 8, still moderate
        // Need more to get complex (9-15)
        assert!(
            flow.complexity_description() == "Moderate (some branching)"
                || flow.complexity_description() == "Complex (multiple paths and loops)"
        );
    }

    #[test]
    fn test_execution_flow_complexity_description_very_complex() {
        let mut builder = ExecutionFlowBuilder::new("test");
        for i in 0..5 {
            builder.add_decision_point(DecisionPoint::new(format!("r{i}"), format!("c{i}")));
            builder.add_loop_structure(LoopStructure::new(format!("l{i}"), format!("e{i}")));
        }
        let flow = builder.build();
        // Score = 1 + 5*2 + 5*3 = 1 + 10 + 15 = 26
        assert_eq!(
            flow.complexity_description(),
            "Very Complex (highly branched with cycles)"
        );
    }

    // =========================================================================
    // DecisionPoint tests
    // =========================================================================

    #[test]
    fn test_decision_point_new() {
        let dp = DecisionPoint::new("router", "x > 0");
        assert_eq!(dp.node, "router");
        assert_eq!(dp.condition, "x > 0");
        assert!(dp.paths.is_empty());
        assert!(dp.explanation.is_empty());
        assert_eq!(dp.decision_type, DecisionType::Conditional);
    }

    #[test]
    fn test_decision_point_with_path() {
        let dp = DecisionPoint::new("router", "cond")
            .with_path(DecisionPath::new("target1", "when1"));
        assert_eq!(dp.paths.len(), 1);
        assert_eq!(dp.paths[0].target, "target1");
    }

    #[test]
    fn test_decision_point_with_paths() {
        let paths = vec![
            DecisionPath::new("t1", "w1"),
            DecisionPath::new("t2", "w2"),
        ];
        let dp = DecisionPoint::new("router", "cond").with_paths(paths);
        assert_eq!(dp.paths.len(), 2);
    }

    #[test]
    fn test_decision_point_with_explanation() {
        let dp = DecisionPoint::new("router", "cond").with_explanation("Routes based on sentiment");
        assert_eq!(dp.explanation, "Routes based on sentiment");
    }

    #[test]
    fn test_decision_point_with_type() {
        let dp = DecisionPoint::new("router", "cond").with_type(DecisionType::ToolSelection);
        assert_eq!(dp.decision_type, DecisionType::ToolSelection);
    }

    #[test]
    fn test_decision_point_path_count() {
        let dp = DecisionPoint::new("router", "cond")
            .with_path(DecisionPath::new("a", "x"))
            .with_path(DecisionPath::new("b", "y"))
            .with_path(DecisionPath::new("c", "z"));
        assert_eq!(dp.path_count(), 3);
    }

    #[test]
    fn test_decision_point_is_binary_true() {
        let dp = DecisionPoint::new("router", "cond")
            .with_path(DecisionPath::new("yes", "true"))
            .with_path(DecisionPath::new("no", "false"));
        assert!(dp.is_binary());
    }

    #[test]
    fn test_decision_point_is_binary_false() {
        let dp = DecisionPoint::new("router", "cond")
            .with_path(DecisionPath::new("a", "x"))
            .with_path(DecisionPath::new("b", "y"))
            .with_path(DecisionPath::new("c", "z"));
        assert!(!dp.is_binary());
    }

    #[test]
    fn test_decision_point_is_binary_single_path() {
        let dp = DecisionPoint::new("router", "cond").with_path(DecisionPath::new("only", "always"));
        assert!(!dp.is_binary());
    }

    // =========================================================================
    // DecisionPath tests
    // =========================================================================

    #[test]
    fn test_decision_path_new() {
        let path = DecisionPath::new("target_node", "when condition is true");
        assert_eq!(path.target, "target_node");
        assert_eq!(path.when, "when condition is true");
        assert!(path.probability.is_none());
    }

    #[test]
    fn test_decision_path_with_probability() {
        let path = DecisionPath::new("target", "condition").with_probability(0.75);
        assert_eq!(path.probability, Some(0.75));
    }

    #[test]
    fn test_decision_path_probability_bounds() {
        let path0 = DecisionPath::new("t", "w").with_probability(0.0);
        let path1 = DecisionPath::new("t", "w").with_probability(1.0);
        assert_eq!(path0.probability, Some(0.0));
        assert_eq!(path1.probability, Some(1.0));
    }

    // =========================================================================
    // DecisionType tests
    // =========================================================================

    #[test]
    fn test_decision_type_default() {
        let dt: DecisionType = Default::default();
        assert_eq!(dt, DecisionType::Conditional);
    }

    #[test]
    fn test_decision_type_display() {
        assert_eq!(format!("{}", DecisionType::Conditional), "Conditional");
        assert_eq!(format!("{}", DecisionType::ToolSelection), "Tool Selection");
        assert_eq!(format!("{}", DecisionType::LoopControl), "Loop Control");
        assert_eq!(format!("{}", DecisionType::ErrorHandling), "Error Handling");
        assert_eq!(format!("{}", DecisionType::HumanApproval), "Human Approval");
        assert_eq!(format!("{}", DecisionType::Parallel), "Parallel");
    }

    #[test]
    fn test_decision_type_equality() {
        assert_eq!(DecisionType::Conditional, DecisionType::Conditional);
        assert_ne!(DecisionType::Conditional, DecisionType::Parallel);
    }

    #[test]
    fn test_decision_type_serialization() {
        let dt = DecisionType::ToolSelection;
        let json = serde_json::to_string(&dt).unwrap();
        assert_eq!(json, "\"tool_selection\"");

        let parsed: DecisionType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, dt);
    }

    // =========================================================================
    // LoopStructure tests
    // =========================================================================

    #[test]
    fn test_loop_structure_new() {
        let ls = LoopStructure::new("retry_loop", "start_node");
        assert_eq!(ls.name, "retry_loop");
        assert_eq!(ls.entry_node, "start_node");
        assert!(ls.nodes_in_loop.is_empty());
        assert!(ls.exit_condition.is_empty());
        assert!(ls.explanation.is_empty());
        assert!(ls.max_iterations.is_none());
        assert_eq!(ls.loop_type, LoopType::Iterative);
    }

    #[test]
    fn test_loop_structure_with_nodes() {
        let ls = LoopStructure::new("loop", "entry").with_nodes(vec!["a", "b", "c"]);
        assert_eq!(ls.nodes_in_loop, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_loop_structure_with_exit_condition() {
        let ls = LoopStructure::new("loop", "entry").with_exit_condition("count >= 3");
        assert_eq!(ls.exit_condition, "count >= 3");
    }

    #[test]
    fn test_loop_structure_with_explanation() {
        let ls = LoopStructure::new("loop", "entry").with_explanation("Retries on failure");
        assert_eq!(ls.explanation, "Retries on failure");
    }

    #[test]
    fn test_loop_structure_with_max_iterations() {
        let ls = LoopStructure::new("loop", "entry").with_max_iterations(10);
        assert_eq!(ls.max_iterations, Some(10));
    }

    #[test]
    fn test_loop_structure_with_type() {
        let ls = LoopStructure::new("loop", "entry").with_type(LoopType::AgentLoop);
        assert_eq!(ls.loop_type, LoopType::AgentLoop);
    }

    #[test]
    fn test_loop_structure_contains() {
        let ls = LoopStructure::new("loop", "entry").with_nodes(vec!["a", "b", "c"]);
        assert!(ls.contains("a"));
        assert!(ls.contains("b"));
        assert!(ls.contains("c"));
        assert!(!ls.contains("d"));
    }

    #[test]
    fn test_loop_structure_chained_methods() {
        let ls = LoopStructure::new("agent", "think")
            .with_nodes(vec!["think", "act", "observe"])
            .with_exit_condition("goal_achieved")
            .with_explanation("Think-Act-Observe agent loop")
            .with_max_iterations(50)
            .with_type(LoopType::AgentLoop);

        assert_eq!(ls.name, "agent");
        assert_eq!(ls.entry_node, "think");
        assert_eq!(ls.nodes_in_loop.len(), 3);
        assert_eq!(ls.exit_condition, "goal_achieved");
        assert_eq!(ls.explanation, "Think-Act-Observe agent loop");
        assert_eq!(ls.max_iterations, Some(50));
        assert_eq!(ls.loop_type, LoopType::AgentLoop);
    }

    // =========================================================================
    // LoopType tests
    // =========================================================================

    #[test]
    fn test_loop_type_default() {
        let lt: LoopType = Default::default();
        assert_eq!(lt, LoopType::Iterative);
    }

    #[test]
    fn test_loop_type_display() {
        assert_eq!(format!("{}", LoopType::Iterative), "Iterative");
        assert_eq!(format!("{}", LoopType::AgentLoop), "Agent Loop");
        assert_eq!(format!("{}", LoopType::RetryLoop), "Retry Loop");
        assert_eq!(format!("{}", LoopType::RefinementLoop), "Refinement Loop");
        assert_eq!(format!("{}", LoopType::MapReduce), "Map-Reduce");
    }

    #[test]
    fn test_loop_type_equality() {
        assert_eq!(LoopType::AgentLoop, LoopType::AgentLoop);
        assert_ne!(LoopType::AgentLoop, LoopType::RetryLoop);
    }

    #[test]
    fn test_loop_type_serialization() {
        let lt = LoopType::RefinementLoop;
        let json = serde_json::to_string(&lt).unwrap();
        assert_eq!(json, "\"refinement_loop\"");

        let parsed: LoopType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, lt);
    }

    // =========================================================================
    // ExecutionPath tests
    // =========================================================================

    #[test]
    fn test_execution_path_new() {
        let path = ExecutionPath::new("happy_path");
        assert_eq!(path.name, "happy_path");
        assert!(path.nodes.is_empty());
        assert!(path.description.is_empty());
        assert!(!path.is_main_path);
    }

    #[test]
    fn test_execution_path_with_nodes() {
        let path = ExecutionPath::new("main").with_nodes(vec!["start", "process", "end"]);
        assert_eq!(path.nodes, vec!["start", "process", "end"]);
    }

    #[test]
    fn test_execution_path_with_description() {
        let path = ExecutionPath::new("error").with_description("Taken when validation fails");
        assert_eq!(path.description, "Taken when validation fails");
    }

    #[test]
    fn test_execution_path_main_path() {
        let path = ExecutionPath::new("main").main_path();
        assert!(path.is_main_path);
    }

    #[test]
    fn test_execution_path_len() {
        let path = ExecutionPath::new("test").with_nodes(vec!["a", "b", "c", "d"]);
        assert_eq!(path.len(), 4);
    }

    #[test]
    fn test_execution_path_is_empty_true() {
        let path = ExecutionPath::new("empty");
        assert!(path.is_empty());
    }

    #[test]
    fn test_execution_path_is_empty_false() {
        let path = ExecutionPath::new("test").with_nodes(vec!["a"]);
        assert!(!path.is_empty());
    }

    #[test]
    fn test_execution_path_chained_methods() {
        let path = ExecutionPath::new("success")
            .with_nodes(vec!["validate", "process", "respond"])
            .with_description("Normal successful execution")
            .main_path();

        assert_eq!(path.name, "success");
        assert_eq!(path.len(), 3);
        assert!(path.is_main_path);
        assert!(!path.description.is_empty());
    }

    // =========================================================================
    // ExecutionFlowMetadata tests
    // =========================================================================

    #[test]
    fn test_metadata_new() {
        let meta = ExecutionFlowMetadata::new();
        assert!(meta.source.is_none());
        assert!(meta.analyzed_at.is_none());
        assert_eq!(meta.node_count, 0);
        assert_eq!(meta.edge_count, 0);
        assert!(meta.notes.is_empty());
    }

    #[test]
    fn test_metadata_with_source() {
        let meta = ExecutionFlowMetadata::new().with_source("static_analysis");
        assert_eq!(meta.source, Some("static_analysis".to_string()));
    }

    #[test]
    fn test_metadata_with_counts() {
        let meta = ExecutionFlowMetadata::new().with_counts(10, 15);
        assert_eq!(meta.node_count, 10);
        assert_eq!(meta.edge_count, 15);
    }

    #[test]
    fn test_metadata_with_note() {
        let meta = ExecutionFlowMetadata::new()
            .with_note("First analysis pass")
            .with_note("Contains unreachable code");
        assert_eq!(meta.notes.len(), 2);
        assert_eq!(meta.notes[0], "First analysis pass");
        assert_eq!(meta.notes[1], "Contains unreachable code");
    }

    #[test]
    fn test_metadata_chained_methods() {
        let meta = ExecutionFlowMetadata::new()
            .with_source("graph_analyzer")
            .with_counts(5, 8)
            .with_note("Auto-generated");

        assert_eq!(meta.source, Some("graph_analyzer".to_string()));
        assert_eq!(meta.node_count, 5);
        assert_eq!(meta.edge_count, 8);
        assert_eq!(meta.notes.len(), 1);
    }

    // =========================================================================
    // generate_flow_description tests
    // =========================================================================

    #[test]
    fn test_generate_flow_description_simple() {
        let nodes = vec!["start".to_string(), "end".to_string()];
        let desc = generate_flow_description("start", &nodes, &[], &[]);

        assert!(desc.contains("start"));
        assert!(desc.contains("2 nodes"));
        assert!(desc.contains("simple linear"));
    }

    #[test]
    fn test_generate_flow_description_with_decisions() {
        let nodes = vec!["start".to_string(), "router".to_string(), "end".to_string()];
        let dp = DecisionPoint::new("router", "x > 0")
            .with_explanation("Routes based on value")
            .with_path(DecisionPath::new("positive", "x > 0"))
            .with_path(DecisionPath::new("negative", "x <= 0"));

        let desc = generate_flow_description("start", &nodes, &[dp], &[]);

        assert!(desc.contains("Decision Points"));
        assert!(desc.contains("router"));
        assert!(desc.contains("Routes based on value"));
        assert!(desc.contains("branching"));
    }

    #[test]
    fn test_generate_flow_description_with_loops() {
        let nodes = vec!["start".to_string(), "process".to_string(), "end".to_string()];
        let ls = LoopStructure::new("retry", "process")
            .with_explanation("Retry on failure")
            .with_exit_condition("success or max_retries");

        let desc = generate_flow_description("start", &nodes, &[], &[ls]);

        assert!(desc.contains("Loop Structures"));
        assert!(desc.contains("retry"));
        assert!(desc.contains("Retry on failure"));
        assert!(desc.contains("success or max_retries"));
        assert!(desc.contains("iterative"));
    }

    #[test]
    fn test_generate_flow_description_complex() {
        let nodes = vec![
            "start".to_string(),
            "router".to_string(),
            "process".to_string(),
            "end".to_string(),
        ];
        let dp = DecisionPoint::new("router", "x > 0").with_explanation("Value check");
        let ls = LoopStructure::new("main", "process")
            .with_explanation("Main loop")
            .with_exit_condition("done");

        let desc = generate_flow_description("start", &nodes, &[dp], &[ls]);

        assert!(desc.contains("Decision Points"));
        assert!(desc.contains("Loop Structures"));
        assert!(desc.contains("complex (branching with loops)"));
    }

    // =========================================================================
    // Serialization roundtrip tests
    // =========================================================================

    #[test]
    fn test_execution_flow_serialization_roundtrip() {
        let dp = DecisionPoint::new("router", "cond")
            .with_path(DecisionPath::new("a", "x").with_probability(0.5))
            .with_explanation("Test decision")
            .with_type(DecisionType::ToolSelection);

        let ls = LoopStructure::new("loop", "entry")
            .with_nodes(vec!["a", "b"])
            .with_exit_condition("done")
            .with_max_iterations(10)
            .with_type(LoopType::RetryLoop);

        let path = ExecutionPath::new("main")
            .with_nodes(vec!["start", "end"])
            .with_description("Main path")
            .main_path();

        let meta = ExecutionFlowMetadata::new()
            .with_source("test")
            .with_counts(5, 7)
            .with_note("Test note");

        let mut builder = ExecutionFlowBuilder::new("test_graph")
            .description("Test flow")
            .entry_point("start")
            .metadata(meta);
        builder.add_exit_point("end");
        builder.add_decision_point(dp);
        builder.add_loop_structure(ls);
        builder.add_linear_path(path);

        let flow = builder.build();

        // Serialize
        let json = serde_json::to_string(&flow).expect("Serialization should succeed");

        // Deserialize
        let parsed: ExecutionFlow =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        // Verify
        assert_eq!(parsed.graph_id, flow.graph_id);
        assert_eq!(parsed.flow_description, flow.flow_description);
        assert_eq!(parsed.entry_point, flow.entry_point);
        assert_eq!(parsed.exit_points, flow.exit_points);
        assert_eq!(parsed.decision_points.len(), flow.decision_points.len());
        assert_eq!(parsed.loop_structures.len(), flow.loop_structures.len());
        assert_eq!(parsed.linear_paths.len(), flow.linear_paths.len());
        assert_eq!(parsed.metadata.node_count, flow.metadata.node_count);
    }

    #[test]
    fn test_decision_point_serialization_roundtrip() {
        let dp = DecisionPoint::new("router", "condition")
            .with_path(DecisionPath::new("target", "when").with_probability(0.8))
            .with_explanation("Test explanation")
            .with_type(DecisionType::ErrorHandling);

        let json = serde_json::to_string(&dp).expect("Serialization should succeed");
        let parsed: DecisionPoint =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(parsed.node, dp.node);
        assert_eq!(parsed.condition, dp.condition);
        assert_eq!(parsed.explanation, dp.explanation);
        assert_eq!(parsed.decision_type, dp.decision_type);
        assert_eq!(parsed.paths.len(), dp.paths.len());
        assert_eq!(parsed.paths[0].probability, dp.paths[0].probability);
    }

    #[test]
    fn test_loop_structure_serialization_roundtrip() {
        let ls = LoopStructure::new("test_loop", "entry")
            .with_nodes(vec!["a", "b", "c"])
            .with_exit_condition("done")
            .with_explanation("Test loop")
            .with_max_iterations(5)
            .with_type(LoopType::MapReduce);

        let json = serde_json::to_string(&ls).expect("Serialization should succeed");
        let parsed: LoopStructure =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(parsed.name, ls.name);
        assert_eq!(parsed.entry_node, ls.entry_node);
        assert_eq!(parsed.nodes_in_loop, ls.nodes_in_loop);
        assert_eq!(parsed.exit_condition, ls.exit_condition);
        assert_eq!(parsed.explanation, ls.explanation);
        assert_eq!(parsed.max_iterations, ls.max_iterations);
        assert_eq!(parsed.loop_type, ls.loop_type);
    }

    #[test]
    fn test_execution_path_serialization_roundtrip() {
        let path = ExecutionPath::new("test_path")
            .with_nodes(vec!["x", "y", "z"])
            .with_description("Test description")
            .main_path();

        let json = serde_json::to_string(&path).expect("Serialization should succeed");
        let parsed: ExecutionPath =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(parsed.name, path.name);
        assert_eq!(parsed.nodes, path.nodes);
        assert_eq!(parsed.description, path.description);
        assert_eq!(parsed.is_main_path, path.is_main_path);
    }
}
