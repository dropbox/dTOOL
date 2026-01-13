// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for debug utilities
// - expect_used: Debug tools can panic on malformed input
// - clone_on_ref_ptr: Debug utilities clone shared state for inspection
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]

//! StateGraph debugging utilities
//!
//! This module provides tools for visualizing StateGraph workflows:
//!
//! - **Mermaid export**: Generate Mermaid diagram syntax for graph visualization
//!
//! For execution tracing and telemetry, use the unified `introspection` module:
//! - `introspection::ExecutionTrace` - Complete execution trace with node execution data
//! - `introspection::ExecutionTraceBuilder` - Builder for constructing traces
//!
//! # Example: Mermaid Export
//!
//! ```rust,ignore
//! use dashflow::{GraphBuilder, END};
//! use dashflow::debug::MermaidExporter;
//!
//! let mut graph = GraphBuilder::new();
//! graph
//!     .add_node("researcher", research_node)
//!     .add_node("writer", writer_node)
//!     .add_edge("researcher", "writer")
//!     .add_edge("writer", END)
//!     .set_entry_point("researcher");
//!
//! let mermaid = MermaidExporter::new(&graph).export();
//! println!("{}", mermaid);
//! // Output:
//! // ```mermaid
//! // graph TD
//! //     __start__([Start])
//! //     __end__([End])
//! //     researcher[researcher]
//! //     writer[writer]
//! //     __start__ --> researcher
//! //     researcher --> writer
//! //     writer --> __end__
//! // ```
//! ```

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::edge::{END, START};
use crate::state::MergeableState;

// ============================================================================
// Mermaid Diagram Export
// ============================================================================

/// Mermaid diagram direction
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MermaidDirection {
    /// Top to bottom (default)
    #[default]
    TopToBottom,
    /// Left to right
    LeftToRight,
    /// Bottom to top
    BottomToTop,
    /// Right to left
    RightToLeft,
}

impl MermaidDirection {
    fn as_str(&self) -> &'static str {
        match self {
            Self::TopToBottom => "TD",
            Self::LeftToRight => "LR",
            Self::BottomToTop => "BT",
            Self::RightToLeft => "RL",
        }
    }
}

/// Mermaid node shape
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MermaidNodeShape {
    /// Rectangle `[text]`
    #[default]
    Rectangle,
    /// Stadium shape `([text])`
    Stadium,
    /// Subroutine `[[text]]`
    Subroutine,
    /// Cylinder [(text)]
    Cylinder,
    /// Circle ((text))
    Circle,
    /// Asymmetric >text]
    Asymmetric,
    /// Rhombus {text}
    Rhombus,
    /// Hexagon {{text}}
    Hexagon,
    /// Parallelogram [/text/]
    Parallelogram,
    /// Trapezoid [/text\]
    Trapezoid,
    /// Double circle (((text)))
    DoubleCircle,
}

impl MermaidNodeShape {
    fn wrap(&self, id: &str, label: &str) -> String {
        match self {
            Self::Rectangle => format!("{}[{}]", id, label),
            Self::Stadium => format!("{}([{}])", id, label),
            Self::Subroutine => format!("{}[[{}]]", id, label),
            Self::Cylinder => format!("{}[({})]", id, label),
            Self::Circle => format!("{}(({}))", id, label),
            Self::Asymmetric => format!("{}>{}]", id, label),
            Self::Rhombus => format!("{}{{{}}}", id, label),
            Self::Hexagon => format!("{}{{{{{}}}}}", id, label),
            Self::Parallelogram => format!("{}[/{}]", id, label),
            Self::Trapezoid => format!("{}[/{}\\]", id, label),
            Self::DoubleCircle => format!("{}((({})))", id, label),
        }
    }
}

/// Configuration for Mermaid diagram export
#[derive(Debug, Clone)]
pub struct MermaidConfig {
    /// Diagram direction
    pub direction: MermaidDirection,
    /// Include markdown code fence
    pub include_fence: bool,
    /// Shape for regular nodes
    pub node_shape: MermaidNodeShape,
    /// Shape for start/end nodes
    pub terminal_shape: MermaidNodeShape,
    /// Shape for conditional nodes
    pub conditional_shape: MermaidNodeShape,
    /// Custom node labels (node_name -> label)
    pub node_labels: HashMap<String, String>,
    /// Custom node styles (node_name -> style class)
    pub node_styles: HashMap<String, String>,
    /// Custom edge labels (from_to -> label)
    pub edge_labels: HashMap<(String, String), String>,
    /// Style definitions
    pub styles: Vec<String>,
    /// Title for the diagram
    pub title: Option<String>,
}

impl Default for MermaidConfig {
    fn default() -> Self {
        Self {
            direction: MermaidDirection::TopToBottom,
            include_fence: true,
            node_shape: MermaidNodeShape::Rectangle,
            terminal_shape: MermaidNodeShape::Stadium,
            conditional_shape: MermaidNodeShape::Rhombus,
            node_labels: HashMap::new(),
            node_styles: HashMap::new(),
            edge_labels: HashMap::new(),
            styles: Vec::new(),
            title: None,
        }
    }
}

impl MermaidConfig {
    /// Create a new configuration with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set diagram direction
    #[must_use]
    pub fn direction(mut self, direction: MermaidDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Include markdown code fence in output
    #[must_use]
    pub fn with_fence(mut self, include: bool) -> Self {
        self.include_fence = include;
        self
    }

    /// Set title for the diagram
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set custom label for a node
    #[must_use]
    pub fn node_label(mut self, node: impl Into<String>, label: impl Into<String>) -> Self {
        self.node_labels.insert(node.into(), label.into());
        self
    }

    /// Set custom style class for a node
    #[must_use]
    pub fn node_style(mut self, node: impl Into<String>, style: impl Into<String>) -> Self {
        self.node_styles.insert(node.into(), style.into());
        self
    }

    /// Set custom label for an edge
    #[must_use]
    pub fn edge_label(
        mut self,
        from: impl Into<String>,
        to: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        self.edge_labels
            .insert((from.into(), to.into()), label.into());
        self
    }

    /// Add a style definition
    #[must_use]
    pub fn add_style(mut self, style: impl Into<String>) -> Self {
        self.styles.push(style.into());
        self
    }
}

/// Graph structure for Mermaid export
///
/// This is a simplified view of the graph without the state type parameter,
/// allowing Mermaid export without needing the full state type.
#[derive(Debug, Clone, Default)]
pub struct GraphStructure {
    /// Node names
    pub nodes: HashSet<String>,
    /// Simple edges (from, to)
    pub edges: Vec<(String, String)>,
    /// Conditional edges (from, routes)
    pub conditional_edges: Vec<(String, HashMap<String, String>)>,
    /// Parallel edges (from, targets)
    pub parallel_edges: Vec<(String, Vec<String>)>,
    /// Entry point node
    pub entry_point: Option<String>,
}

impl GraphStructure {
    /// Create a new empty graph structure
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node to the structure
    pub fn add_node(&mut self, name: impl Into<String>) -> &mut Self {
        self.nodes.insert(name.into());
        self
    }

    /// Add a simple edge
    pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) -> &mut Self {
        self.edges.push((from.into(), to.into()));
        self
    }

    /// Add a conditional edge
    pub fn add_conditional_edge(
        &mut self,
        from: impl Into<String>,
        routes: HashMap<String, String>,
    ) -> &mut Self {
        self.conditional_edges.push((from.into(), routes));
        self
    }

    /// Add a parallel edge
    pub fn add_parallel_edge(
        &mut self,
        from: impl Into<String>,
        targets: Vec<String>,
    ) -> &mut Self {
        self.parallel_edges.push((from.into(), targets));
        self
    }

    /// Set the entry point
    pub fn set_entry_point(&mut self, entry: impl Into<String>) -> &mut Self {
        self.entry_point = Some(entry.into());
        self
    }

    /// Export to Mermaid diagram syntax
    pub fn to_mermaid(&self, config: &MermaidConfig) -> String {
        let mut output = String::new();

        // Code fence
        if config.include_fence {
            writeln!(output, "```mermaid").unwrap();
        }

        // Title
        if let Some(title) = &config.title {
            writeln!(output, "---").unwrap();
            writeln!(output, "title: {}", title).unwrap();
            writeln!(output, "---").unwrap();
        }

        // Graph type and direction
        writeln!(output, "graph {}", config.direction.as_str()).unwrap();

        // Special nodes (start/end)
        writeln!(output, "    {}", config.terminal_shape.wrap(START, "Start")).unwrap();
        writeln!(output, "    {}", config.terminal_shape.wrap(END, "End")).unwrap();

        // Regular nodes
        for node in &self.nodes {
            let label = config
                .node_labels
                .get(node)
                .cloned()
                .unwrap_or_else(|| node.clone());
            let escaped_label = escape_mermaid_label(&label);
            writeln!(
                output,
                "    {}",
                config
                    .node_shape
                    .wrap(&sanitize_node_id(node), &escaped_label)
            )
            .unwrap();
        }

        // Entry point edge
        if let Some(entry) = &self.entry_point {
            writeln!(output, "    {} --> {}", START, sanitize_node_id(entry)).unwrap();
        }

        // Simple edges
        for (from, to) in &self.edges {
            let from_id = sanitize_node_id(from);
            let to_id = sanitize_node_id(to);
            let label = config.edge_labels.get(&(from.clone(), to.clone()));
            if let Some(label) = label {
                writeln!(output, "    {} -->|{}| {}", from_id, label, to_id).unwrap();
            } else {
                writeln!(output, "    {} --> {}", from_id, to_id).unwrap();
            }
        }

        // Conditional edges
        for (from, routes) in &self.conditional_edges {
            let from_id = sanitize_node_id(from);
            for (condition, target) in routes {
                let to_id = sanitize_node_id(target);
                writeln!(output, "    {} -->|{}| {}", from_id, condition, to_id).unwrap();
            }
        }

        // Parallel edges
        for (from, targets) in &self.parallel_edges {
            let from_id = sanitize_node_id(from);
            for target in targets {
                let to_id = sanitize_node_id(target);
                writeln!(output, "    {} -.->|parallel| {}", from_id, to_id).unwrap();
            }
        }

        // Node styles
        for (node, style) in &config.node_styles {
            writeln!(output, "    class {} {}", sanitize_node_id(node), style).unwrap();
        }

        // Style definitions
        for style in &config.styles {
            writeln!(output, "    {}", style).unwrap();
        }

        if config.include_fence {
            writeln!(output, "```").unwrap();
        }

        output
    }

    /// Export to Graphviz DOT format
    ///
    /// Generates a DOT file that can be rendered with Graphviz tools like `dot` or `neato`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow::debug::GraphStructure;
    ///
    /// let mut structure = GraphStructure::new();
    /// structure.add_node("a").add_node("b").add_edge("a", "b");
    /// let dot = structure.to_dot();
    /// // Output:
    /// // digraph G {
    /// //     rankdir=TB;
    /// //     node [shape=box];
    /// //     "__start__" [shape=ellipse, label="Start"];
    /// //     "__end__" [shape=ellipse, label="End"];
    /// //     "a" [label="a"];
    /// //     "b" [label="b"];
    /// //     "a" -> "b";
    /// // }
    /// ```
    pub fn to_dot(&self) -> String {
        let mut output = String::new();

        writeln!(output, "digraph G {{").unwrap();
        writeln!(output, "    rankdir=TB;").unwrap();
        writeln!(output, "    node [shape=box];").unwrap();
        writeln!(output).unwrap();

        // Special nodes (start/end)
        writeln!(
            output,
            "    \"{}\" [shape=ellipse, label=\"Start\"];",
            START
        )
        .unwrap();
        writeln!(output, "    \"{}\" [shape=ellipse, label=\"End\"];", END).unwrap();
        writeln!(output).unwrap();

        // Regular nodes
        for node in &self.nodes {
            let escaped = escape_dot_string(node);
            writeln!(output, "    \"{}\" [label=\"{}\"];", escaped, escaped).unwrap();
        }
        writeln!(output).unwrap();

        // Entry point edge
        if let Some(entry) = &self.entry_point {
            writeln!(
                output,
                "    \"{}\" -> \"{}\";",
                START,
                escape_dot_string(entry)
            )
            .unwrap();
        }

        // Simple edges
        for (from, to) in &self.edges {
            writeln!(
                output,
                "    \"{}\" -> \"{}\";",
                escape_dot_string(from),
                escape_dot_string(to)
            )
            .unwrap();
        }

        // Conditional edges
        for (from, routes) in &self.conditional_edges {
            for (condition, target) in routes {
                writeln!(
                    output,
                    "    \"{}\" -> \"{}\" [label=\"{}\"];",
                    escape_dot_string(from),
                    escape_dot_string(target),
                    escape_dot_string(condition)
                )
                .unwrap();
            }
        }

        // Parallel edges
        for (from, targets) in &self.parallel_edges {
            for target in targets {
                writeln!(
                    output,
                    "    \"{}\" -> \"{}\" [style=dashed, label=\"parallel\"];",
                    escape_dot_string(from),
                    escape_dot_string(target)
                )
                .unwrap();
            }
        }

        writeln!(output, "}}").unwrap();

        output
    }

    /// Export to ASCII art representation
    ///
    /// Generates a simple text-based visualization of the graph structure.
    /// Useful for terminal output and documentation.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow::debug::GraphStructure;
    ///
    /// let mut structure = GraphStructure::new();
    /// structure.add_node("a").add_node("b").add_edge("a", "b");
    /// structure.set_entry_point("a");
    /// let ascii = structure.to_ascii();
    /// // Output:
    /// // Graph Structure
    /// // ==============
    /// // Entry: [Start] -> a
    /// //
    /// // Nodes: a, b
    /// //
    /// // Edges:
    /// //   a -> b
    /// ```
    pub fn to_ascii(&self) -> String {
        let mut output = String::new();

        writeln!(output, "Graph Structure").unwrap();
        writeln!(output, "==============").unwrap();

        // Entry point
        if let Some(entry) = &self.entry_point {
            writeln!(output, "Entry: [Start] -> {}", entry).unwrap();
        } else {
            writeln!(output, "Entry: (none)").unwrap();
        }
        writeln!(output).unwrap();

        // Nodes
        if self.nodes.is_empty() {
            writeln!(output, "Nodes: (none)").unwrap();
        } else {
            let mut sorted_nodes: Vec<_> = self.nodes.iter().collect();
            sorted_nodes.sort();
            writeln!(
                output,
                "Nodes: {}",
                sorted_nodes
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .unwrap();
        }
        writeln!(output).unwrap();

        // Simple edges
        if self.edges.is_empty()
            && self.conditional_edges.is_empty()
            && self.parallel_edges.is_empty()
        {
            writeln!(output, "Edges: (none)").unwrap();
        } else {
            writeln!(output, "Edges:").unwrap();

            // Simple edges
            for (from, to) in &self.edges {
                writeln!(output, "  {} -> {}", from, to).unwrap();
            }

            // Conditional edges
            for (from, routes) in &self.conditional_edges {
                for (condition, target) in routes {
                    writeln!(output, "  {} --[{}]--> {}", from, condition, target).unwrap();
                }
            }

            // Parallel edges
            for (from, targets) in &self.parallel_edges {
                for target in targets {
                    writeln!(output, "  {} -.parallel.-> {}", from, target).unwrap();
                }
            }
        }

        output
    }
}

/// Escape special characters for DOT strings
fn escape_dot_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Sanitize node ID for Mermaid (remove special characters)
fn sanitize_node_id(id: &str) -> String {
    id.replace(['-', '.', ' ', '/', '\\'], "_")
}

/// Escape special characters in Mermaid labels
fn escape_mermaid_label(label: &str) -> String {
    label
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Extension trait for StateGraph to enable Mermaid export
pub trait MermaidExport {
    /// Extract graph structure for Mermaid export
    fn to_graph_structure(&self) -> GraphStructure;

    /// Export to Mermaid diagram with default config
    fn to_mermaid(&self) -> String {
        self.to_graph_structure()
            .to_mermaid(&MermaidConfig::default())
    }

    /// Export to Mermaid diagram with custom config
    fn to_mermaid_with_config(&self, config: &MermaidConfig) -> String {
        self.to_graph_structure().to_mermaid(config)
    }
}

// ============================================================================
// Execution Tracing
// ============================================================================

/// A single step in the execution trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    /// Step number (0-indexed)
    pub step_number: u32,
    /// Node that was executed
    pub node: String,
    /// Timestamp when execution started
    pub started_at: SystemTime,
    /// Duration of execution
    pub duration: Duration,
    /// Edge taken to reach next node (if any)
    pub edge_taken: Option<EdgeTaken>,
    /// State snapshot before execution (serialized JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_before: Option<String>,
    /// State snapshot after execution (serialized JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_after: Option<String>,
    /// Error message if node failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Information about an edge taken during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeTaken {
    /// Edge type
    pub edge_type: TracedEdgeType,
    /// Source node
    pub from: String,
    /// Target node(s)
    pub to: Vec<String>,
    /// Condition result (for conditional edges)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_result: Option<String>,
}

/// Type of edge in a trace
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TracedEdgeType {
    /// Simple unconditional edge
    Simple,
    /// Conditional edge
    Conditional,
    /// Parallel edge
    Parallel,
}

// ============================================================================
// StateGraph Integration
// ============================================================================

use crate::graph::StateGraph;

impl<S> MermaidExport for StateGraph<S>
where
    S: MergeableState,
{
    fn to_graph_structure(&self) -> GraphStructure {
        let mut structure = GraphStructure::new();

        // Add nodes
        for node_name in self.node_names() {
            structure.add_node(node_name);
        }

        // Add simple edges
        for edge in self.get_edges() {
            structure.add_edge(edge.from.as_str(), edge.to.as_str());
        }

        // Add conditional edges
        for edge in self.get_conditional_edges() {
            // Convert Arc<String> values back to String for debug structure
            let routes: HashMap<String, String> = edge
                .routes
                .iter()
                .map(|(k, v)| (k.clone(), (**v).clone()))
                .collect();
            structure.add_conditional_edge(edge.from.as_str(), routes);
        }

        // Add parallel edges
        for edge in self.get_parallel_edges() {
            structure.add_parallel_edge(edge.from.as_str(), edge.to.as_ref().clone());
        }

        // Set entry point
        if let Some(entry) = self.get_entry_point() {
            structure.set_entry_point(entry);
        }

        structure
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_structure_to_mermaid() {
        let mut structure = GraphStructure::new();
        structure
            .add_node("researcher")
            .add_node("writer")
            .add_edge("researcher", "writer")
            .add_edge("writer", END)
            .set_entry_point("researcher");

        let mermaid = structure.to_mermaid(&MermaidConfig::default());

        assert!(mermaid.contains("```mermaid"));
        assert!(mermaid.contains("graph TD"));
        assert!(mermaid.contains("researcher[researcher]"));
        assert!(mermaid.contains("writer[writer]"));
        assert!(mermaid.contains("__start__ --> researcher"));
        assert!(mermaid.contains("researcher --> writer"));
        assert!(mermaid.contains("writer --> __end__"));
    }

    #[test]
    fn test_mermaid_with_conditional_edges() {
        let mut structure = GraphStructure::new();
        structure
            .add_node("router")
            .add_node("node_a")
            .add_node("node_b");

        let mut routes = HashMap::new();
        routes.insert("option_a".to_string(), "node_a".to_string());
        routes.insert("option_b".to_string(), "node_b".to_string());
        structure.add_conditional_edge("router", routes);
        structure.set_entry_point("router");

        let mermaid = structure.to_mermaid(&MermaidConfig::default());

        assert!(
            mermaid.contains("router -->|option_a| node_a")
                || mermaid.contains("router -->|option_b| node_b")
        );
    }

    #[test]
    fn test_mermaid_with_parallel_edges() {
        let mut structure = GraphStructure::new();
        structure
            .add_node("splitter")
            .add_node("worker_1")
            .add_node("worker_2");
        structure.add_parallel_edge(
            "splitter",
            vec!["worker_1".to_string(), "worker_2".to_string()],
        );
        structure.set_entry_point("splitter");

        let mermaid = structure.to_mermaid(&MermaidConfig::default());

        assert!(mermaid.contains("splitter -.->|parallel| worker_1"));
        assert!(mermaid.contains("splitter -.->|parallel| worker_2"));
    }

    #[test]
    fn test_mermaid_config() {
        let config = MermaidConfig::new()
            .direction(MermaidDirection::LeftToRight)
            .with_fence(false)
            .title("My Graph")
            .node_label("n1", "Node One")
            .edge_label("n1", "n2", "transition");

        assert_eq!(config.direction, MermaidDirection::LeftToRight);
        assert!(!config.include_fence);
        assert_eq!(config.title, Some("My Graph".to_string()));
        assert_eq!(config.node_labels.get("n1"), Some(&"Node One".to_string()));
        assert_eq!(
            config
                .edge_labels
                .get(&("n1".to_string(), "n2".to_string())),
            Some(&"transition".to_string())
        );
    }

    #[test]
    fn test_sanitize_node_id() {
        assert_eq!(sanitize_node_id("node-1"), "node_1");
        assert_eq!(sanitize_node_id("path/to/node"), "path_to_node");
        assert_eq!(sanitize_node_id("node.name"), "node_name");
    }

    #[test]
    fn test_escape_mermaid_label() {
        assert_eq!(escape_mermaid_label("a<b>c"), "a&lt;b&gt;c");
        assert_eq!(escape_mermaid_label("a\"b\"c"), "a&quot;b&quot;c");
    }

    #[test]
    fn test_graph_structure_to_dot() {
        let mut structure = GraphStructure::new();
        structure
            .add_node("researcher")
            .add_node("writer")
            .add_edge("researcher", "writer")
            .add_edge("writer", END)
            .set_entry_point("researcher");

        let dot = structure.to_dot();

        assert!(dot.contains("digraph G {"));
        assert!(dot.contains("rankdir=TB;"));
        assert!(dot.contains("\"__start__\" [shape=ellipse, label=\"Start\"];"));
        assert!(dot.contains("\"__end__\" [shape=ellipse, label=\"End\"];"));
        assert!(dot.contains("\"researcher\" [label=\"researcher\"];"));
        assert!(dot.contains("\"writer\" [label=\"writer\"];"));
        assert!(dot.contains("\"__start__\" -> \"researcher\";"));
        assert!(dot.contains("\"researcher\" -> \"writer\";"));
        assert!(dot.contains("\"writer\" -> \"__end__\";"));
        assert!(dot.contains("}"));
    }

    #[test]
    fn test_dot_with_conditional_edges() {
        let mut structure = GraphStructure::new();
        structure
            .add_node("router")
            .add_node("node_a")
            .add_node("node_b");

        let mut routes = HashMap::new();
        routes.insert("option_a".to_string(), "node_a".to_string());
        routes.insert("option_b".to_string(), "node_b".to_string());
        structure.add_conditional_edge("router", routes);

        let dot = structure.to_dot();

        // Check that conditional edges have labels
        assert!(
            dot.contains("\"router\" -> \"node_a\" [label=\"option_a\"];")
                || dot.contains("\"router\" -> \"node_b\" [label=\"option_b\"];")
        );
    }

    #[test]
    fn test_dot_with_parallel_edges() {
        let mut structure = GraphStructure::new();
        structure
            .add_node("splitter")
            .add_node("worker_1")
            .add_node("worker_2");
        structure.add_parallel_edge(
            "splitter",
            vec!["worker_1".to_string(), "worker_2".to_string()],
        );

        let dot = structure.to_dot();

        assert!(dot.contains("\"splitter\" -> \"worker_1\" [style=dashed, label=\"parallel\"];"));
        assert!(dot.contains("\"splitter\" -> \"worker_2\" [style=dashed, label=\"parallel\"];"));
    }

    #[test]
    fn test_graph_structure_to_ascii() {
        let mut structure = GraphStructure::new();
        structure
            .add_node("researcher")
            .add_node("writer")
            .add_edge("researcher", "writer")
            .add_edge("writer", END)
            .set_entry_point("researcher");

        let ascii = structure.to_ascii();

        assert!(ascii.contains("Graph Structure"));
        assert!(ascii.contains("=============="));
        assert!(ascii.contains("Entry: [Start] -> researcher"));
        assert!(ascii.contains("Nodes:"));
        assert!(ascii.contains("researcher"));
        assert!(ascii.contains("writer"));
        assert!(ascii.contains("Edges:"));
        assert!(ascii.contains("researcher -> writer"));
        assert!(ascii.contains("writer -> __end__"));
    }

    #[test]
    fn test_ascii_with_conditional_edges() {
        let mut structure = GraphStructure::new();
        structure.add_node("router").add_node("target");

        let mut routes = HashMap::new();
        routes.insert("condition".to_string(), "target".to_string());
        structure.add_conditional_edge("router", routes);

        let ascii = structure.to_ascii();

        assert!(ascii.contains("router --[condition]--> target"));
    }

    #[test]
    fn test_ascii_with_parallel_edges() {
        let mut structure = GraphStructure::new();
        structure.add_node("splitter").add_node("worker");
        structure.add_parallel_edge("splitter", vec!["worker".to_string()]);

        let ascii = structure.to_ascii();

        assert!(ascii.contains("splitter -.parallel.-> worker"));
    }

    #[test]
    fn test_ascii_empty_graph() {
        let structure = GraphStructure::new();
        let ascii = structure.to_ascii();

        assert!(ascii.contains("Entry: (none)"));
        assert!(ascii.contains("Nodes: (none)"));
        assert!(ascii.contains("Edges: (none)"));
    }

    #[test]
    fn test_escape_dot_string() {
        assert_eq!(escape_dot_string("normal"), "normal");
        assert_eq!(escape_dot_string("with\"quote"), "with\\\"quote");
        assert_eq!(escape_dot_string("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_dot_string("both\"and\\"), "both\\\"and\\\\");
    }
}
