// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Extension trait for adding DashOptimize nodes to StateGraph
//!
//! This module provides ergonomic builder methods for integrating optimizable
//! LLM nodes (Predict, ChainOfThought, ReAct) into DashFlow workflows.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::prelude::*;
//! use dashflow::optimize::DspyGraphExt;
//!
//! #[derive(Clone, Serialize, Deserialize)]
//! struct State {
//!     question: String,
//!     answer: String,
//! }
//!
//! let mut graph = StateGraph::new();
//!
//! // Add optimizable chain-of-thought node
//! graph
//!     .add_chain_of_thought_node("solve", llm_client)
//!     .with_signature("question -> answer", "Solve math problems step by step")
//!     .add_edge("solve", END);
//!
//! let app = graph.compile()?;
//! ```

use crate::core::language_models::ChatModel;
use crate::graph::StateGraph;
use crate::optimize::modules::{ChainOfThoughtNode, ReActNode, Tool};
use crate::optimize::signature::{make_signature, Signature};
use crate::state::MergeableState;
use std::sync::Arc;
use tracing::{debug, warn};

/// Extension trait for StateGraph to support DashOptimize nodes
///
/// This trait adds convenience methods for integrating optimizable LLM nodes
/// into DashFlow workflows. All methods return builders that allow fluent
/// configuration before adding the node to the graph.
///
/// # Core Methods
///
/// - `add_llm_node()` - Basic signature-based LLM node (Predict equivalent)
/// - `add_chain_of_thought_node()` - Reasoning + answer node
/// - `add_react_node()` - Tool-use agent node
///
/// # Example: Multi-node workflow
///
/// ```rust,ignore
/// let mut graph = StateGraph::new();
///
/// // Research node (ChainOfThought for complex reasoning)
/// graph
///     .add_chain_of_thought_node("research", llm)
///     .with_signature(
///         "topic -> findings",
///         "Research the topic and summarize key findings"
///     );
///
/// // Tool-use node (ReAct for web search + calculator)
/// graph
///     .add_react_node("search", llm)
///     .with_tools(vec![web_search_tool, calculator_tool])
///     .with_instruction("Find relevant information using available tools");
///
/// // Final answer node (basic Predict)
/// graph
///     .add_llm_node("synthesize", llm)
///     .with_signature(
///         "topic, findings -> report",
///         "Synthesize research into comprehensive report"
///     );
///
/// graph
///     .add_edge("research", "search")
///     .add_edge("search", "synthesize")
///     .add_edge("synthesize", END);
///
/// let app = graph.compile()?;
/// ```
pub trait DspyGraphExt<S: MergeableState> {
    /// Add a basic signature-based LLM node (Predict equivalent)
    ///
    /// This creates a node that takes structured inputs, builds a prompt from
    /// a signature, calls the LLM, and parses structured outputs back into state.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the node in the graph
    /// * `llm` - Chat model to use for generation
    ///
    /// # Returns
    ///
    /// `LLMNodeBuilder` for fluent configuration
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// graph
    ///     .add_llm_node("classify", llm_client)
    ///     .with_signature("text -> category", "Classify text sentiment")
    ///     .add_edge("classify", END);
    /// ```
    fn add_llm_node(
        &mut self,
        name: impl Into<String>,
        llm: Arc<dyn ChatModel>,
    ) -> LLMNodeBuilder<'_, S>;

    /// Add a Chain-of-Thought reasoning node
    ///
    /// This creates a node that generates reasoning before producing an answer.
    /// The signature is automatically extended with a "reasoning" field.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the node in the graph
    /// * `llm` - Chat model to use for generation
    ///
    /// # Returns
    ///
    /// `ChainOfThoughtBuilder` for fluent configuration
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// graph
    ///     .add_chain_of_thought_node("solve", llm_client)
    ///     .with_signature(
    ///         "problem -> solution",
    ///         "Solve math problems step by step"
    ///     )
    ///     .add_edge("solve", END);
    /// ```
    fn add_chain_of_thought_node(
        &mut self,
        name: impl Into<String>,
        llm: Arc<dyn ChatModel>,
    ) -> ChainOfThoughtBuilder<'_, S>;

    /// Add a ReAct tool-use agent node
    ///
    /// This creates a node that iterates through Thought → Action → Observation
    /// cycles until it reaches a final answer. Useful for tasks requiring
    /// tool interaction (search, calculator, APIs).
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the node in the graph
    /// * `llm` - Chat model to use for generation
    ///
    /// # Returns
    ///
    /// `ReActBuilder` for fluent configuration
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tools = vec![
    ///     Arc::new(SearchTool::new()) as Arc<dyn Tool>,
    ///     Arc::new(CalculatorTool::new()) as Arc<dyn Tool>,
    /// ];
    ///
    /// graph
    ///     .add_react_node("agent", llm_client)
    ///     .with_tools(tools)
    ///     .with_signature("question -> answer", "Answer questions using available tools")
    ///     .with_max_iterations(5)
    ///     .add_edge("agent", END);
    /// ```
    fn add_react_node(
        &mut self,
        name: impl Into<String>,
        llm: Arc<dyn ChatModel>,
    ) -> ReActBuilder<'_, S>;
}

/// Builder for basic LLM nodes (Predict equivalent)
pub struct LLMNodeBuilder<'a, S: MergeableState> {
    graph: &'a mut StateGraph<S>,
    name: String,
    llm: Arc<dyn ChatModel>,
    signature: Option<Signature>,
}

impl<'a, S: MergeableState> LLMNodeBuilder<'a, S> {
    fn new(graph: &'a mut StateGraph<S>, name: String, llm: Arc<dyn ChatModel>) -> Self {
        Self {
            graph,
            name,
            llm,
            signature: None,
        }
    }

    /// Set the signature for this node
    ///
    /// # Arguments
    ///
    /// * `sig_str` - Signature string (e.g., "question -> answer")
    /// * `instruction` - Task instruction for the LLM
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.with_signature(
    ///     "question, context -> answer",
    ///     "Answer questions based on context"
    /// )
    /// ```
    #[must_use]
    pub fn with_signature(mut self, sig_str: &str, instruction: &str) -> Self {
        match make_signature(sig_str, instruction) {
            Ok(sig) => self.signature = Some(sig),
            Err(e) => {
                warn!(
                    node = %self.name,
                    sig_str = %sig_str,
                    error = %e,
                    "Failed to parse signature; will use default on add()"
                );
                self.signature = None;
            }
        }
        self
    }

    /// Finalize and add the node to the graph
    ///
    /// Returns mutable reference to the graph for fluent chaining.
    #[allow(clippy::expect_used)] // Default signature is a compile-time constant
    pub fn add(self) -> &'a mut StateGraph<S> {
        use crate::optimize::llm_node::LLMNode;

        if let Some(signature) = self.signature {
            let node = LLMNode::new(signature, self.llm);
            self.graph.add_node(self.name, node)
        } else {
            // Default signature if none provided
            debug!(
                node = %self.name,
                default_sig = "input -> output",
                "Using default signature for LLM node"
            );
            let default_sig = make_signature("input -> output", "Process the input")
                .expect("Default signature should be valid");
            let node = LLMNode::new(default_sig, self.llm);
            self.graph.add_node(self.name, node)
        }
    }
}

/// Builder for Chain-of-Thought nodes
pub struct ChainOfThoughtBuilder<'a, S: MergeableState> {
    graph: &'a mut StateGraph<S>,
    name: String,
    llm: Arc<dyn ChatModel>,
    signature: Option<Signature>,
}

impl<'a, S: MergeableState> ChainOfThoughtBuilder<'a, S> {
    fn new(graph: &'a mut StateGraph<S>, name: String, llm: Arc<dyn ChatModel>) -> Self {
        Self {
            graph,
            name,
            llm,
            signature: None,
        }
    }

    /// Set the signature for this node
    ///
    /// The signature will be automatically extended with a "reasoning" field
    /// before the final output.
    ///
    /// # Arguments
    ///
    /// * `sig_str` - Signature string (e.g., "question -> answer")
    /// * `instruction` - Task instruction for the LLM
    #[must_use]
    pub fn with_signature(mut self, sig_str: &str, instruction: &str) -> Self {
        match make_signature(sig_str, instruction) {
            Ok(sig) => self.signature = Some(sig),
            Err(e) => {
                warn!(
                    node = %self.name,
                    sig_str = %sig_str,
                    error = %e,
                    "Failed to parse signature; will use default on add()"
                );
                self.signature = None;
            }
        }
        self
    }

    /// Finalize and add the node to the graph
    #[allow(clippy::expect_used)] // Default signature is a compile-time constant
    pub fn add(self) -> &'a mut StateGraph<S> {
        // Create ChainOfThoughtNode (signature, llm)
        if let Some(signature) = self.signature {
            let node = ChainOfThoughtNode::new(signature, self.llm);
            self.graph.add_node(self.name, node)
        } else {
            // Default signature if none provided
            debug!(
                node = %self.name,
                default_sig = "input -> output",
                "Using default signature for ChainOfThought node"
            );
            let default_sig = make_signature("input -> output", "Think step by step")
                .expect("Default signature should be valid");
            let node = ChainOfThoughtNode::new(default_sig, self.llm);
            self.graph.add_node(self.name, node)
        }
    }
}

/// Builder for ReAct agent nodes
pub struct ReActBuilder<'a, S: MergeableState> {
    graph: &'a mut StateGraph<S>,
    name: String,
    llm: Arc<dyn ChatModel>,
    tools: Vec<Arc<dyn Tool>>,
    signature: Option<Signature>,
    max_iterations: usize,
}

impl<'a, S: MergeableState> ReActBuilder<'a, S> {
    fn new(graph: &'a mut StateGraph<S>, name: String, llm: Arc<dyn ChatModel>) -> Self {
        Self {
            graph,
            name,
            llm,
            tools: Vec::new(),
            signature: None,
            max_iterations: 5,
        }
    }

    /// Set the tools available to this agent
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.with_tools(vec![
    ///     Arc::new(SearchTool::new()),
    ///     Arc::new(CalculatorTool::new()),
    /// ])
    /// ```
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools = tools;
        self
    }

    /// Set the signature for the agent
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.with_signature(
    ///     "question -> answer",
    ///     "Answer questions using available tools"
    /// )
    /// ```
    #[must_use]
    pub fn with_signature(mut self, sig_str: &str, instruction: &str) -> Self {
        match make_signature(sig_str, instruction) {
            Ok(sig) => self.signature = Some(sig),
            Err(e) => {
                warn!(
                    node = %self.name,
                    sig_str = %sig_str,
                    error = %e,
                    "Failed to parse signature; will use default on add()"
                );
                self.signature = None;
            }
        }
        self
    }

    /// Set maximum iterations for the ReAct loop
    ///
    /// Default: 5 iterations
    #[must_use]
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Finalize and add the node to the graph
    #[allow(clippy::expect_used)] // Default signature is a compile-time constant
    pub fn add(self) -> &'a mut StateGraph<S> {
        let signature = self.signature.unwrap_or_else(|| {
            debug!(
                node = %self.name,
                default_sig = "question -> answer",
                "Using default signature for ReAct node"
            );
            make_signature(
                "question -> answer",
                "Answer questions using available tools",
            )
            .expect("Default signature should be valid")
        });

        let node = ReActNode::new(signature, self.tools, self.max_iterations, self.llm);
        self.graph.add_node(self.name, node)
    }
}

impl<S: MergeableState> DspyGraphExt<S> for StateGraph<S> {
    fn add_llm_node(
        &mut self,
        name: impl Into<String>,
        llm: Arc<dyn ChatModel>,
    ) -> LLMNodeBuilder<'_, S> {
        LLMNodeBuilder::new(self, name.into(), llm)
    }

    fn add_chain_of_thought_node(
        &mut self,
        name: impl Into<String>,
        llm: Arc<dyn ChatModel>,
    ) -> ChainOfThoughtBuilder<'_, S> {
        ChainOfThoughtBuilder::new(self, name.into(), llm)
    }

    fn add_react_node(
        &mut self,
        name: impl Into<String>,
        llm: Arc<dyn ChatModel>,
    ) -> ReActBuilder<'_, S> {
        ReActBuilder::new(self, name.into(), llm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::callbacks::CallbackManager;
    use crate::core::error::Result as CoreResult;
    use crate::core::language_models::{ChatGeneration, ChatResult, ToolChoice, ToolDefinition};
    use crate::core::messages::BaseMessage;
    use crate::core::tools::{Tool as OptTool, ToolInput};
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct TestState {
        question: String,
        answer: String,
        reasoning: Option<String>,
    }

    impl crate::state::MergeableState for TestState {
        fn merge(&mut self, other: &Self) {
            if !other.question.is_empty() {
                self.question = other.question.clone();
            }
            if !other.answer.is_empty() {
                self.answer = other.answer.clone();
            }
            if other.reasoning.is_some() {
                self.reasoning = other.reasoning.clone();
            }
        }
    }

    // Mock ChatModel for testing
    struct MockChatModel {
        response: String,
    }

    impl MockChatModel {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
            }
        }
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _callbacks: Option<&CallbackManager>,
        ) -> crate::core::Result<ChatResult> {
            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: crate::core::messages::Message::ai(self.response.clone()),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock_chat"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    // Mock Tool for ReAct tests
    #[derive(Debug)]
    struct MockTool {
        name: String,
    }

    impl MockTool {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl OptTool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A mock tool for testing"
        }

        fn args_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            })
        }

        async fn _call(&self, _input: ToolInput) -> CoreResult<String> {
            Ok("mock result".to_string())
        }
    }

    #[test]
    fn test_dashoptimize_graph_ext_trait_compiles() {
        // This test just verifies the trait and builders compile correctly
        let _graph: StateGraph<TestState> = StateGraph::new();

        // Verify trait is in scope and methods exist
        // (compilation is the test)
    }

    #[test]
    fn test_llm_node_builder_creation() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("test response"));

        // Create builder
        let builder = graph.add_llm_node("test_node", llm);

        // Verify builder has expected fields
        assert_eq!(builder.name, "test_node");
        assert!(builder.signature.is_none());
    }

    #[test]
    fn test_llm_node_builder_with_signature() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("test response"));

        // Create builder with signature
        let builder = graph
            .add_llm_node("qa_node", llm)
            .with_signature("question -> answer", "Answer questions");

        // Signature should be set
        assert!(builder.signature.is_some());
        let sig = builder.signature.as_ref().unwrap();
        assert_eq!(sig.instructions, "Answer questions");
    }

    #[test]
    fn test_llm_node_builder_add_to_graph() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("test response"));

        // Add node to graph
        let graph_ref = graph
            .add_llm_node("qa_node", llm)
            .with_signature("question -> answer", "Answer questions")
            .add();

        // Verify node was added by checking graph has the node
        // The add() returns mutable reference to graph
        assert!(std::ptr::eq(graph_ref, &graph));
    }

    #[test]
    fn test_llm_node_builder_default_signature() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("test response"));

        // Add node without signature - should use default
        let _graph_ref = graph.add_llm_node("default_node", llm).add();

        // Node should be added with default signature (test passes if no panic)
    }

    #[test]
    fn test_chain_of_thought_builder_creation() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("reasoning + answer"));

        // Create CoT builder
        let builder = graph.add_chain_of_thought_node("cot_node", llm);

        assert_eq!(builder.name, "cot_node");
        assert!(builder.signature.is_none());
    }

    #[test]
    fn test_chain_of_thought_builder_with_signature() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("reasoning + answer"));

        let builder = graph
            .add_chain_of_thought_node("solver", llm)
            .with_signature("question -> answer", "Solve step by step");

        assert!(builder.signature.is_some());
        let sig = builder.signature.as_ref().unwrap();
        assert_eq!(sig.instructions, "Solve step by step");
    }

    #[test]
    fn test_chain_of_thought_builder_add_to_graph() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("reasoning + answer"));

        let _graph_ref = graph
            .add_chain_of_thought_node("solver", llm)
            .with_signature("question -> answer", "Solve step by step")
            .add();

        // Node should be added (test passes if no panic)
    }

    #[test]
    fn test_chain_of_thought_default_signature() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("reasoning + answer"));

        // Add node without signature - should use default "input -> output"
        let _graph_ref = graph.add_chain_of_thought_node("default_cot", llm).add();

        // Node should be added with default signature (test passes if no panic)
    }

    #[test]
    fn test_react_builder_creation() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("thought + action"));

        let builder = graph.add_react_node("agent", llm);

        assert_eq!(builder.name, "agent");
        assert!(builder.tools.is_empty());
        assert!(builder.signature.is_none());
        assert_eq!(builder.max_iterations, 5); // Default
    }

    #[test]
    fn test_react_builder_with_tools() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("thought + action"));

        let tools: Vec<Arc<dyn OptTool>> = vec![
            Arc::new(MockTool::new("search")),
            Arc::new(MockTool::new("calculator")),
        ];

        let builder = graph.add_react_node("agent", llm).with_tools(tools);

        assert_eq!(builder.tools.len(), 2);
    }

    #[test]
    fn test_react_builder_with_signature() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("thought + action"));

        let builder = graph
            .add_react_node("agent", llm)
            .with_signature("question -> answer", "Use tools to find the answer");

        assert!(builder.signature.is_some());
        let sig = builder.signature.as_ref().unwrap();
        assert_eq!(sig.instructions, "Use tools to find the answer");
    }

    #[test]
    fn test_react_builder_with_max_iterations() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("thought + action"));

        let builder = graph.add_react_node("agent", llm).with_max_iterations(10);

        assert_eq!(builder.max_iterations, 10);
    }

    #[test]
    fn test_react_builder_fluent_chain() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("thought + action"));

        let tools: Vec<Arc<dyn OptTool>> = vec![Arc::new(MockTool::new("search"))];

        // Full fluent chain
        let _graph_ref = graph
            .add_react_node("agent", llm)
            .with_tools(tools)
            .with_signature("question -> answer", "Find the answer")
            .with_max_iterations(3)
            .add();

        // Node should be added (test passes if no panic)
    }

    #[test]
    fn test_react_builder_default_signature() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("thought + action"));

        // Add node without signature - should use default
        let _graph_ref = graph.add_react_node("agent", llm).add();

        // Node should be added with default signature (test passes if no panic)
    }

    #[test]
    fn test_multiple_nodes_same_graph() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm1 = Arc::new(MockChatModel::new("response1"));
        let llm2 = Arc::new(MockChatModel::new("response2"));
        let llm3 = Arc::new(MockChatModel::new("response3"));

        // Add multiple different node types to same graph
        graph
            .add_llm_node("predict", llm1)
            .with_signature("question -> answer", "Predict")
            .add();

        graph
            .add_chain_of_thought_node("reason", llm2)
            .with_signature("problem -> solution", "Reason step by step")
            .add();

        graph.add_react_node("agent", llm3).add();

        // All nodes should be added (test passes if no panic)
    }

    #[test]
    fn test_invalid_signature_string() {
        let mut graph: StateGraph<TestState> = StateGraph::new();
        let llm = Arc::new(MockChatModel::new("test"));

        // Invalid signature string (no arrow) - should result in None
        let builder = graph
            .add_llm_node("test", llm)
            .with_signature("invalid signature", "Instructions");

        // make_signature returns None for invalid strings
        assert!(builder.signature.is_none());
    }
}
