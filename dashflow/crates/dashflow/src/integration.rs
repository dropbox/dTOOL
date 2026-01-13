// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Integration with DashFlow components
//!
//! This module provides adapters to use DashFlow Agents, Chains, and Tools
//! as graph nodes.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::{StateGraph, RunnableNode};
//! use dashflow::core::Agent;
//!
//! let agent = Agent::new(/* ... */);
//! let agent_node = RunnableNode::new("agent", agent);
//!
//! let mut graph = StateGraph::new();
//! graph.add_node("agent", agent_node);
//! ```

use crate::core::config::RunnableConfig;
use crate::core::runnable::Runnable;
use crate::core::tools::{Tool, ToolInput};
use async_trait::async_trait;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::error::Result;
use crate::node::Node;

/// Adapter that wraps any `Runnable` as a DashFlow `Node`
///
/// This allows using Agents, Chains, and other Runnable components
/// directly as graph nodes.
///
/// # Type Parameters
///
/// * `R` - The Runnable type (Agent, Chain, etc.)
/// * `S` - The state type that must match the Runnable's Input/Output
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::RunnableNode;
/// use dashflow::core::Chain;
///
/// let chain = Chain::new(/* ... */);
/// let node = RunnableNode::new("my_chain", chain);
///
/// // Use in graph
/// graph.add_node("process", node);
/// ```
pub struct RunnableNode<R, S>
where
    R: Runnable<Input = S, Output = S>,
{
    name: String,
    runnable: Arc<R>,
    config: Option<RunnableConfig>,
    _phantom: PhantomData<S>,
}

impl<R, S> RunnableNode<R, S>
where
    R: Runnable<Input = S, Output = S>,
{
    /// Create a new RunnableNode
    ///
    /// # Arguments
    ///
    /// * `name` - Name for this node
    /// * `runnable` - The Runnable to wrap (Agent, Chain, etc.)
    pub fn new(name: impl Into<String>, runnable: R) -> Self {
        Self {
            name: name.into(),
            runnable: Arc::new(runnable),
            config: None,
            _phantom: PhantomData,
        }
    }

    /// Set the RunnableConfig for this node
    ///
    /// This allows configuring callbacks, tags, metadata, etc.
    #[must_use]
    pub fn with_config(mut self, config: RunnableConfig) -> Self {
        self.config = Some(config);
        self
    }
}

#[async_trait]
impl<R, S> Node<S> for RunnableNode<R, S>
where
    R: Runnable<Input = S, Output = S> + 'static,
    S: Send + Sync + 'static,
{
    async fn execute(&self, state: S) -> Result<S> {
        let output = self
            .runnable
            .invoke(state, self.config.clone())
            .await
            .map_err(|e| crate::Error::NodeExecution {
                node: self.name.clone(),
                source: Box::new(e),
            })?;
        Ok(output)
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Adapter for Agents that extracts state from agent execution
///
/// Agents often work with message-based state. This adapter helps
/// integrate agents into graphs with custom state types.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::AgentNode;
/// use dashflow::core::Agent;
///
/// #[derive(Clone)]
/// struct MyState {
///     messages: Vec<Message>,
///     context: String,
/// }
///
/// let agent = Agent::new(/* ... */);
/// let agent_node = AgentNode::new(
///     "agent",
///     agent,
///     |state: MyState| state.messages,          // Extract input
///     |state, messages| {                        // Update state
///         MyState { messages, ..state }
///     }
/// );
/// ```
pub struct AgentNode<A, S, I, FIn, FOut>
where
    A: Runnable<Input = I>,
    FIn: Fn(S) -> I + Send + Sync,
    FOut: Fn(S, A::Output) -> S + Send + Sync,
{
    name: String,
    agent: Arc<A>,
    extract_input: FIn,
    update_state: FOut,
    config: Option<RunnableConfig>,
    _phantom: PhantomData<(S, I)>,
}

impl<A, S, I, FIn, FOut> AgentNode<A, S, I, FIn, FOut>
where
    A: Runnable<Input = I>,
    FIn: Fn(S) -> I + Send + Sync,
    FOut: Fn(S, A::Output) -> S + Send + Sync,
{
    /// Create a new AgentNode
    ///
    /// # Arguments
    ///
    /// * `name` - Name for this node
    /// * `agent` - The Agent to wrap
    /// * `extract_input` - Function to extract agent input from state
    /// * `update_state` - Function to update state with agent output
    pub fn new(name: impl Into<String>, agent: A, extract_input: FIn, update_state: FOut) -> Self {
        Self {
            name: name.into(),
            agent: Arc::new(agent),
            extract_input,
            update_state,
            config: None,
            _phantom: PhantomData,
        }
    }

    /// Set the RunnableConfig for this agent
    #[must_use]
    pub fn with_config(mut self, config: RunnableConfig) -> Self {
        self.config = Some(config);
        self
    }
}

#[async_trait]
impl<A, S, I, FIn, FOut> Node<S> for AgentNode<A, S, I, FIn, FOut>
where
    A: Runnable<Input = I> + 'static,
    S: Send + Sync + Clone + 'static,
    I: Send + Sync + 'static,
    A::Output: Send + Sync + 'static,
    FIn: Fn(S) -> I + Send + Sync + 'static,
    FOut: Fn(S, A::Output) -> S + Send + Sync + 'static,
{
    async fn execute(&self, state: S) -> Result<S> {
        // Extract input from state
        let input = (self.extract_input)(state.clone());

        // Run agent
        let output = self
            .agent
            .invoke(input, self.config.clone())
            .await
            .map_err(|e| crate::Error::NodeExecution {
                node: self.name.clone(),
                source: Box::new(e),
            })?;

        // Update state with output
        let new_state = (self.update_state)(state, output);
        Ok(new_state)
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Adapter for Tools that execute within graph context
///
/// Tools are executed as graph nodes, with their output stored in state.
/// This enables tool-using workflows where graph state accumulates tool results.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::ToolNode;
/// use dashflow::core::Tool;
///
/// #[derive(Clone)]
/// struct MyState {
///     tool_input: String,
///     tool_output: String,
/// }
///
/// let search_tool = /* create tool */;
/// let tool_node = ToolNode::new(
///     search_tool,
///     |state: MyState| ToolInput::String(state.tool_input),
///     |mut state, output| {
///         state.tool_output = output;
///         state
///     }
/// );
///
/// graph.add_node("search", tool_node);
/// ```
pub struct ToolNode<T, S, FIn, FOut>
where
    T: Tool,
    FIn: Fn(S) -> ToolInput + Send + Sync,
    FOut: Fn(S, String) -> S + Send + Sync,
{
    tool: Arc<T>,
    extract_input: FIn,
    update_state: FOut,
    _phantom: PhantomData<S>,
}

impl<T, S, FIn, FOut> ToolNode<T, S, FIn, FOut>
where
    T: Tool,
    FIn: Fn(S) -> ToolInput + Send + Sync,
    FOut: Fn(S, String) -> S + Send + Sync,
{
    /// Create a new ToolNode
    ///
    /// # Arguments
    ///
    /// * `tool` - The Tool to execute
    /// * `extract_input` - Function to extract tool input from state
    /// * `update_state` - Function to update state with tool output
    pub fn new(tool: T, extract_input: FIn, update_state: FOut) -> Self {
        Self {
            tool: Arc::new(tool),
            extract_input,
            update_state,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<T, S, FIn, FOut> Node<S> for ToolNode<T, S, FIn, FOut>
where
    T: Tool + 'static,
    S: Send + Sync + Clone + 'static,
    FIn: Fn(S) -> ToolInput + Send + Sync + 'static,
    FOut: Fn(S, String) -> S + Send + Sync + 'static,
{
    async fn execute(&self, state: S) -> Result<S> {
        // Extract tool input from state
        let input = (self.extract_input)(state.clone());

        // Execute tool
        let tool_name = self.tool.name();
        let output = self
            .tool
            ._call(input)
            .await
            .map_err(|e| crate::Error::NodeExecution {
                node: tool_name.to_string(),
                source: Box::new(std::io::Error::other(e)),
            })?;

        // Update state with output
        let new_state = (self.update_state)(state, output);
        Ok(new_state)
    }

    fn name(&self) -> String {
        self.tool.name().to_string()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Helper function to determine if tools should be called based on message state.
///
/// This is a common conditional routing function used with DashFlow.
/// It checks if the last message in the state is an AI message with tool calls.
///
/// # Arguments
///
/// * `messages` - A slice of messages from the conversation state
///
/// # Returns
///
/// * `"tools"` - If the last AI message contains tool calls
/// * `END` - If no tool calls are present (conversation should end)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::{StateGraph, tools_condition, END};
/// use dashflow::core::messages::Message;
///
/// // In your graph setup:
/// let mut routes = std::collections::HashMap::new();
/// routes.insert("tools".to_string(), "tools".to_string());
/// routes.insert(END.to_string(), END.to_string());
///
/// graph.add_conditional_edges(
///     "assistant",
///     |state: &AgentState| tools_condition(&state.messages),
///     routes,
/// );
/// ```
pub fn tools_condition(messages: &[crate::core::messages::Message]) -> &'static str {
    use crate::core::messages::Message;
    use crate::END;

    if let Some(Message::AI { tool_calls, .. }) = messages.last() {
        if !tool_calls.is_empty() {
            return "tools";
        }
    }
    END
}

/// Automatically execute tool calls from the last AI message and return tool response messages.
///
/// This is the Rust equivalent of Python's `ToolNode` from upstream DashFlow prebuilt.
/// It extracts tool calls from the last message, executes matching tools, and returns
/// tool response messages that can be appended to the conversation state.
///
/// # Arguments
///
/// * `messages` - The conversation message history
/// * `tools` - Available tools to execute
///
/// # Returns
///
/// A vector of Tool messages containing the results of executed tool calls.
/// Returns an empty vector if there are no tool calls to execute.
///
/// # Errors
///
/// Returns an error if:
/// - A tool call references a tool that doesn't exist
/// - A tool execution fails
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::auto_tool_executor;
/// use dashflow::core::messages::Message;
/// use std::sync::Arc;
///
/// // In your tools node:
/// async fn tools_node(
///     mut state: AgentState,
///     tools: Vec<Arc<dyn Tool>>,
/// ) -> Result<AgentState> {
///     // Execute tools and get response messages
///     let tool_messages = auto_tool_executor(&state.messages, &tools).await?;
///
///     // Append responses to state
///     state.messages.extend(tool_messages);
///     Ok(state)
/// }
/// ```
pub async fn auto_tool_executor(
    messages: &[crate::core::messages::Message],
    tools: &[Arc<dyn Tool>],
) -> Result<Vec<crate::core::messages::Message>> {
    use crate::core::messages::{Message, MessageContent};
    use serde_json::json;

    fn truncate_utf8(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            return s.to_string();
        }
        let target_len = max_len.saturating_sub(3);
        let truncate_at = s
            .char_indices()
            .take_while(|(idx, _)| *idx < target_len)
            .last()
            .map(|(idx, c)| idx + c.len_utf8())
            .unwrap_or(0);
        format!("{}...", &s[..truncate_at])
    }

    let stream_writer = crate::stream::get_stream_writer();

    // Get the last message
    let last_message = match messages.last() {
        Some(msg) => msg,
        None => return Ok(Vec::new()),
    };

    // Extract tool calls from AI message
    let tool_calls = match last_message {
        Message::AI { tool_calls, .. } => tool_calls,
        _ => return Ok(Vec::new()),
    };

    if tool_calls.is_empty() {
        return Ok(Vec::new());
    }

    // Build a map of tool names to tools for fast lookup
    let tool_map: std::collections::HashMap<&str, &Arc<dyn Tool>> =
        tools.iter().map(|t| (t.name(), t)).collect();

    // Build deduplication set: (tool_name, serialized_args)
    // Check recent history (last 10 messages) to avoid repeated identical searches
    // Skip the last message (current one) since we're about to execute its tool calls
    let mut seen_calls = std::collections::HashSet::new();
    for msg in messages.iter().rev().skip(1).take(10) {
        if let Message::AI {
            tool_calls: prev_calls,
            ..
        } = msg
        {
            for prev_call in prev_calls {
                let key = (
                    prev_call.name.clone(),
                    serde_json::to_string(&prev_call.args).unwrap_or_default(),
                );
                seen_calls.insert(key);
            }
        }
    }

    // Execute each tool call and collect results
    let mut tool_messages = Vec::with_capacity(tool_calls.len());

    for tool_call in tool_calls {
        if let Some(writer) = &stream_writer {
            writer.write(json!({
                "type": "tool_call_start",
                "tool_call_id": tool_call.id,
                "name": tool_call.name,
                "args": tool_call.args,
            }));
        }

        // Check for duplicate tool calls
        let call_key = (
            tool_call.name.clone(),
            serde_json::to_string(&tool_call.args).unwrap_or_default(),
        );
        if seen_calls.contains(&call_key) {
            // Skip duplicate - create cached response message
            let cached_result = format!(
                "=== CACHED RESULT (tool '{}' already called with these arguments) ===\n\n\
                 This tool was already called with these arguments. Please use the previous output.\n\n\
                 === END CACHED ===",
                tool_call.name
            );

            let tool_message = Message::Tool {
                content: MessageContent::Text(cached_result),
                tool_call_id: tool_call.id.clone(),
                artifact: None,
                status: Some("cached".to_string()),
                fields: Default::default(),
            };
            if let Some(writer) = &stream_writer {
                writer.write(json!({
                    "type": "tool_call_end",
                    "tool_call_id": tool_call.id,
                    "name": tool_call.name,
                    "status": "cached",
                }));
            }
            tool_messages.push(tool_message);
            continue;
        }
        // Find the matching tool
        let tool = tool_map.get(tool_call.name.as_str()).ok_or_else(|| {
            if let Some(writer) = &stream_writer {
                writer.write(json!({
                    "type": "tool_call_end",
                    "tool_call_id": tool_call.id,
                    "name": tool_call.name,
                    "status": "not_found",
                }));
            }
            crate::Error::NodeNotFound(format!(
                "Tool '{}' not found in available tools",
                tool_call.name
            ))
        })?;

        // Convert args to ToolInput
        let input = if tool_call.args.is_string() {
            ToolInput::String(tool_call.args.as_str().unwrap_or("").to_string())
        } else {
            ToolInput::Structured(tool_call.args.clone())
        };

        // Execute the tool
        let result = tool._call(input).await.map_err(|e| {
            if let Some(writer) = &stream_writer {
                writer.write(json!({
                    "type": "tool_call_end",
                    "tool_call_id": tool_call.id,
                    "name": tool_call.name,
                    "status": "error",
                    "error": e.to_string(),
                }));
            }
            crate::Error::NodeExecution {
                node: format!("tool:{}", tool_call.name),
                source: Box::new(std::io::Error::other(e)),
            }
        })?;

        if let Some(writer) = &stream_writer {
            writer.write(json!({
                "type": "tool_call_end",
                "tool_call_id": tool_call.id,
                "name": tool_call.name,
                "status": "success",
                "result_len": result.len(),
                "result_preview": truncate_utf8(&result, 400),
            }));
        }

        // Format tool result to be obvious to LLM
        let formatted_result = format!(
            "=== TOOL OUTPUT FROM '{}' ===\n\n\
             {}\n\n\
             === END TOOL OUTPUT ===\n\n\
             IMPORTANT: Use this tool output in your response.",
            tool_call.name, result
        );

        // Create tool message with formatted result
        let tool_message = Message::Tool {
            content: MessageContent::Text(formatted_result),
            tool_call_id: tool_call.id.clone(),
            artifact: None,
            status: Some("success".to_string()),
            fields: Default::default(),
        };

        tool_messages.push(tool_message);
    }

    Ok(tool_messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::RunnableConfig;
    use crate::core::runnable::Runnable;
    use crate::core::tools::{Tool, ToolInput};

    #[derive(Clone, Debug, PartialEq)]
    struct TestState {
        value: i32,
    }

    struct DoubleRunnable;

    #[async_trait]
    impl Runnable for DoubleRunnable {
        type Input = TestState;
        type Output = TestState;

        async fn invoke(
            &self,
            input: Self::Input,
            _config: Option<RunnableConfig>,
        ) -> crate::core::Result<Self::Output> {
            Ok(TestState {
                value: input.value * 2,
            })
        }
    }

    #[tokio::test]
    async fn test_runnable_node() {
        let runnable = DoubleRunnable;
        let node = RunnableNode::new("double", runnable);

        let state = TestState { value: 5 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 10);
        assert_eq!(node.name(), "double");
    }

    #[tokio::test]
    async fn test_agent_node() {
        struct IncrementAgent;

        #[async_trait]
        impl Runnable for IncrementAgent {
            type Input = i32;
            type Output = i32;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Ok(input + 1)
            }
        }

        let agent = IncrementAgent;
        let node = AgentNode::new(
            "increment",
            agent,
            |state: TestState| state.value,
            |_state, output| TestState { value: output },
        );

        let state = TestState { value: 10 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 11);
        assert_eq!(node.name(), "increment");
    }

    #[tokio::test]
    async fn test_runnable_node_with_config() {
        let runnable = DoubleRunnable;
        let config = RunnableConfig::default().with_tag("test");
        let node = RunnableNode::new("double", runnable).with_config(config);

        let state = TestState { value: 3 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 6);
    }

    #[tokio::test]
    async fn test_tool_node() {
        struct UppercaseTool;

        #[async_trait]
        impl Tool for UppercaseTool {
            fn name(&self) -> &str {
                "uppercase"
            }

            fn description(&self) -> &str {
                "Converts text to uppercase"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::String(s) => Ok(s.to_uppercase()),
                    _ => Err(crate::core::Error::Other(
                        "Expected string input".to_string(),
                    )),
                }
            }
        }

        #[derive(Clone)]
        struct ToolState {
            input: String,
            output: String,
        }

        let tool = UppercaseTool;
        let node = ToolNode::new(
            tool,
            |state: ToolState| ToolInput::String(state.input),
            |mut state, output| {
                state.output = output;
                state
            },
        );

        let state = ToolState {
            input: "hello world".to_string(),
            output: String::new(),
        };

        let result = node.execute(state).await.unwrap();
        assert_eq!(result.output, "HELLO WORLD");
        assert_eq!(node.name(), "uppercase");
    }

    // ===== RunnableNode Tests =====

    #[tokio::test]
    async fn test_runnable_node_new() {
        let runnable = DoubleRunnable;
        let node = RunnableNode::new("test", runnable);
        assert_eq!(node.name(), "test");
    }

    #[tokio::test]
    async fn test_runnable_node_name_from_string() {
        let runnable = DoubleRunnable;
        let node = RunnableNode::new("my_node".to_string(), runnable);
        assert_eq!(node.name(), "my_node");
    }

    #[tokio::test]
    async fn test_runnable_node_execute_preserves_state() {
        struct IdentityRunnable;

        #[async_trait]
        impl Runnable for IdentityRunnable {
            type Input = TestState;
            type Output = TestState;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Ok(input)
            }
        }

        let node = RunnableNode::new("identity", IdentityRunnable);
        let state = TestState { value: 42 };
        let result = node.execute(state.clone()).await.unwrap();
        assert_eq!(result, state);
    }

    #[tokio::test]
    async fn test_runnable_node_error_propagation() {
        struct FailingRunnable;

        #[async_trait]
        impl Runnable for FailingRunnable {
            type Input = TestState;
            type Output = TestState;

            async fn invoke(
                &self,
                _input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Err(crate::core::Error::Other("Intentional failure".to_string()))
            }
        }

        let node = RunnableNode::new("failing", FailingRunnable);
        let state = TestState { value: 1 };
        let result = node.execute(state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_runnable_node_with_config_preserves_name() {
        let runnable = DoubleRunnable;
        let config = RunnableConfig::default();
        let node = RunnableNode::new("configured", runnable).with_config(config);
        assert_eq!(node.name(), "configured");
    }

    #[tokio::test]
    async fn test_runnable_node_config_with_tags() {
        let runnable = DoubleRunnable;
        let config = RunnableConfig::default()
            .with_tag("integration")
            .with_tag("test");
        let node = RunnableNode::new("tagged", runnable).with_config(config);

        let state = TestState { value: 7 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 14);
    }

    #[tokio::test]
    async fn test_runnable_node_config_with_metadata() {
        let runnable = DoubleRunnable;
        let mut config = RunnableConfig::default();
        config.metadata.insert(
            "key".to_string(),
            serde_json::Value::String("value".to_string()),
        );
        let node = RunnableNode::new("metadata", runnable).with_config(config);

        let state = TestState { value: 2 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 4);
    }

    // ===== AgentNode Tests =====

    #[tokio::test]
    async fn test_agent_node_new() {
        struct TestAgent;

        #[async_trait]
        impl Runnable for TestAgent {
            type Input = i32;
            type Output = i32;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Ok(input)
            }
        }

        let agent = TestAgent;
        let node = AgentNode::new(
            "agent",
            agent,
            |state: TestState| state.value,
            |_state, output| TestState { value: output },
        );

        assert_eq!(node.name(), "agent");
    }

    #[tokio::test]
    async fn test_agent_node_extract_input() {
        struct MultiplyAgent;

        #[async_trait]
        impl Runnable for MultiplyAgent {
            type Input = i32;
            type Output = i32;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Ok(input * 3)
            }
        }

        let agent = MultiplyAgent;
        let node = AgentNode::new(
            "multiply",
            agent,
            |state: TestState| state.value,
            |_state, output| TestState { value: output },
        );

        let state = TestState { value: 4 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 12);
    }

    #[tokio::test]
    async fn test_agent_node_update_state() {
        struct ConstantAgent;

        #[async_trait]
        impl Runnable for ConstantAgent {
            type Input = i32;
            type Output = i32;

            async fn invoke(
                &self,
                _input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Ok(999)
            }
        }

        let agent = ConstantAgent;
        let node = AgentNode::new(
            "constant",
            agent,
            |state: TestState| state.value,
            |mut state, output| {
                state.value = output;
                state
            },
        );

        let state = TestState { value: 1 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 999);
    }

    #[tokio::test]
    async fn test_agent_node_state_preservation() {
        #[derive(Clone)]
        struct ComplexState {
            value: i32,
            message: String,
        }

        struct SimpleAgent;

        #[async_trait]
        impl Runnable for SimpleAgent {
            type Input = i32;
            type Output = i32;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Ok(input + 10)
            }
        }

        let agent = SimpleAgent;
        let node = AgentNode::new(
            "simple",
            agent,
            |state: ComplexState| state.value,
            |mut state, output| {
                state.value = output;
                state
            },
        );

        let state = ComplexState {
            value: 5,
            message: "preserved".to_string(),
        };

        let result = node.execute(state.clone()).await.unwrap();
        assert_eq!(result.value, 15);
        assert_eq!(result.message, "preserved");
    }

    #[tokio::test]
    async fn test_agent_node_with_config() {
        struct AddAgent;

        #[async_trait]
        impl Runnable for AddAgent {
            type Input = i32;
            type Output = i32;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Ok(input + 5)
            }
        }

        let agent = AddAgent;
        let config = RunnableConfig::default().with_tag("agent_test");
        let node = AgentNode::new(
            "add",
            agent,
            |state: TestState| state.value,
            |_state, output| TestState { value: output },
        )
        .with_config(config);

        let state = TestState { value: 20 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 25);
    }

    #[tokio::test]
    async fn test_agent_node_error_propagation() {
        struct FailingAgent;

        #[async_trait]
        impl Runnable for FailingAgent {
            type Input = i32;
            type Output = i32;

            async fn invoke(
                &self,
                _input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Err(crate::core::Error::Other("Agent error".to_string()))
            }
        }

        let agent = FailingAgent;
        let node = AgentNode::new(
            "failing",
            agent,
            |state: TestState| state.value,
            |_state, output| TestState { value: output },
        );

        let state = TestState { value: 1 };
        let result = node.execute(state).await;
        assert!(result.is_err());
    }

    // ===== ToolNode Tests =====

    #[tokio::test]
    async fn test_tool_node_name_from_tool() {
        struct NamedTool;

        #[async_trait]
        impl Tool for NamedTool {
            fn name(&self) -> &str {
                "my_tool"
            }

            fn description(&self) -> &str {
                "A test tool"
            }

            async fn _call(&self, _input: ToolInput) -> crate::core::Result<String> {
                Ok("result".to_string())
            }
        }

        #[derive(Clone)]
        struct SimpleState {
            data: String,
        }

        let tool = NamedTool;
        let node = ToolNode::new(
            tool,
            |state: SimpleState| ToolInput::String(state.data),
            |mut state, output| {
                state.data = output;
                state
            },
        );

        assert_eq!(node.name(), "my_tool");
    }

    #[tokio::test]
    async fn test_tool_node_string_input() {
        struct ReverseTool;

        #[async_trait]
        impl Tool for ReverseTool {
            fn name(&self) -> &str {
                "reverse"
            }

            fn description(&self) -> &str {
                "Reverses a string"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::String(s) => Ok(s.chars().rev().collect()),
                    _ => Err(crate::core::Error::Other("Expected string".to_string())),
                }
            }
        }

        #[derive(Clone)]
        struct TextState {
            input: String,
            output: String,
        }

        let tool = ReverseTool;
        let node = ToolNode::new(
            tool,
            |state: TextState| ToolInput::String(state.input),
            |mut state, output| {
                state.output = output;
                state
            },
        );

        let state = TextState {
            input: "hello".to_string(),
            output: String::new(),
        };

        let result = node.execute(state).await.unwrap();
        assert_eq!(result.output, "olleh");
    }

    #[tokio::test]
    async fn test_tool_node_structured_input() {
        struct LengthTool;

        #[async_trait]
        impl Tool for LengthTool {
            fn name(&self) -> &str {
                "length"
            }

            fn description(&self) -> &str {
                "Returns string length"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::Structured(value) => {
                        if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
                            Ok(text.len().to_string())
                        } else {
                            Ok("0".to_string())
                        }
                    }
                    _ => Err(crate::core::Error::Other(
                        "Expected structured input".to_string(),
                    )),
                }
            }
        }

        #[derive(Clone)]
        struct StructuredState {
            text: String,
            length: String,
        }

        let tool = LengthTool;
        let node = ToolNode::new(
            tool,
            |state: StructuredState| {
                let json = serde_json::json!({"text": state.text});
                ToolInput::Structured(json)
            },
            |mut state, output| {
                state.length = output;
                state
            },
        );

        let state = StructuredState {
            text: "test string".to_string(),
            length: String::new(),
        };

        let result = node.execute(state).await.unwrap();
        assert_eq!(result.length, "11");
    }

    #[tokio::test]
    async fn test_tool_node_error_handling() {
        struct ErrorTool;

        #[async_trait]
        impl Tool for ErrorTool {
            fn name(&self) -> &str {
                "error"
            }

            fn description(&self) -> &str {
                "Always fails"
            }

            async fn _call(&self, _input: ToolInput) -> crate::core::Result<String> {
                Err(crate::core::Error::Other("Tool failed".to_string()))
            }
        }

        #[derive(Clone)]
        struct SimpleState {
            data: String,
        }

        let tool = ErrorTool;
        let node = ToolNode::new(
            tool,
            |state: SimpleState| ToolInput::String(state.data),
            |mut state, output| {
                state.data = output;
                state
            },
        );

        let state = SimpleState {
            data: "test".to_string(),
        };

        let result = node.execute(state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_node_state_transformation() {
        struct AppendTool;

        #[async_trait]
        impl Tool for AppendTool {
            fn name(&self) -> &str {
                "append"
            }

            fn description(&self) -> &str {
                "Appends suffix"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::String(s) => Ok(format!("{}_suffix", s)),
                    _ => Err(crate::core::Error::Other("Expected string".to_string())),
                }
            }
        }

        #[derive(Clone)]
        struct StateWithHistory {
            current: String,
            history: Vec<String>,
        }

        let tool = AppendTool;
        let node = ToolNode::new(
            tool,
            |state: StateWithHistory| ToolInput::String(state.current.clone()),
            |mut state, output| {
                state.history.push(state.current.clone());
                state.current = output;
                state
            },
        );

        let state = StateWithHistory {
            current: "value".to_string(),
            history: vec![],
        };

        let result = node.execute(state).await.unwrap();
        assert_eq!(result.current, "value_suffix");
        assert_eq!(result.history, vec!["value"]);
    }

    #[tokio::test]
    async fn test_tool_node_empty_string_input() {
        struct EchoTool;

        #[async_trait]
        impl Tool for EchoTool {
            fn name(&self) -> &str {
                "echo"
            }

            fn description(&self) -> &str {
                "Echoes input"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::String(s) => Ok(s),
                    _ => Err(crate::core::Error::Other("Expected string".to_string())),
                }
            }
        }

        #[derive(Clone)]
        struct SimpleState {
            data: String,
        }

        let tool = EchoTool;
        let node = ToolNode::new(
            tool,
            |state: SimpleState| ToolInput::String(state.data),
            |mut state, output| {
                state.data = output;
                state
            },
        );

        let state = SimpleState {
            data: String::new(),
        };

        let result = node.execute(state).await.unwrap();
        assert_eq!(result.data, "");
    }

    // ===== tools_condition Tests =====

    #[test]
    fn test_tools_condition_no_tool_calls() {
        use crate::core::messages::Message;
        use crate::END;

        let messages = vec![Message::human("Hello"), Message::ai("Hi there!")];

        let result = tools_condition(&messages);
        assert_eq!(result, END);
    }

    #[test]
    fn test_tools_condition_with_tool_calls() {
        use crate::core::messages::{Message, ToolCall};

        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "search".to_string(),
            args: serde_json::json!({"query": "test"}),
            tool_type: "function".to_string(),
            index: None,
        };

        let messages = vec![
            Message::human("Search for something"),
            Message::AI {
                content: crate::core::messages::MessageContent::Text("Let me search".to_string()),
                tool_calls: vec![tool_call],
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: Default::default(),
            },
        ];

        let result = tools_condition(&messages);
        assert_eq!(result, "tools");
    }

    #[test]
    fn test_tools_condition_empty_tool_calls() {
        use crate::core::messages::Message;
        use crate::END;

        let messages = vec![
            Message::human("Hello"),
            Message::AI {
                content: crate::core::messages::MessageContent::Text("Response".to_string()),
                tool_calls: vec![], // Empty tool calls
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: Default::default(),
            },
        ];

        let result = tools_condition(&messages);
        assert_eq!(result, END);
    }

    #[test]
    fn test_tools_condition_empty_messages() {
        use crate::END;

        let messages: Vec<crate::core::messages::Message> = vec![];

        let result = tools_condition(&messages);
        assert_eq!(result, END);
    }

    #[test]
    fn test_tools_condition_last_message_not_ai() {
        use crate::core::messages::Message;
        use crate::END;

        let messages = vec![
            Message::human("Question"),
            Message::ai("Answer"),
            Message::human("Follow-up"),
        ];

        let result = tools_condition(&messages);
        assert_eq!(result, END);
    }

    // ===== auto_tool_executor Tests =====

    #[tokio::test]
    async fn test_auto_tool_executor_no_messages() {
        let messages: Vec<crate::core::messages::Message> = vec![];
        let tools: Vec<Arc<dyn Tool>> = vec![];

        let result = super::auto_tool_executor(&messages, &tools).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_auto_tool_executor_no_tool_calls() {
        use crate::core::messages::Message;

        let messages = vec![Message::human("Hello"), Message::ai("Hi there!")];
        let tools: Vec<Arc<dyn Tool>> = vec![];

        let result = super::auto_tool_executor(&messages, &tools).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_auto_tool_executor_with_single_tool_call() {
        use crate::core::messages::{Message, MessageContent, ToolCall};

        struct SearchTool;

        #[async_trait]
        impl Tool for SearchTool {
            fn name(&self) -> &str {
                "search"
            }

            fn description(&self) -> &str {
                "Search tool"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::String(query) => Ok(format!("Search results for: {}", query)),
                    _ => Err(crate::core::Error::Other("Expected string".to_string())),
                }
            }
        }

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "search".to_string(),
            args: serde_json::Value::String("rust programming".to_string()),
            tool_type: "function".to_string(),
            index: None,
        };

        let messages = vec![
            Message::human("Search for rust programming"),
            Message::AI {
                content: MessageContent::Text("Let me search for that".to_string()),
                tool_calls: vec![tool_call],
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: Default::default(),
            },
        ];

        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(SearchTool)];

        let result = super::auto_tool_executor(&messages, &tools).await.unwrap();
        assert_eq!(result.len(), 1);

        match &result[0] {
            Message::Tool {
                content,
                tool_call_id,
                status,
                ..
            } => {
                assert_eq!(
                    content.as_text(),
                    "=== TOOL OUTPUT FROM 'search' ===\n\nSearch results for: rust programming\n\n=== END TOOL OUTPUT ===\n\nIMPORTANT: Use this tool output in your response."
                );
                assert_eq!(tool_call_id, "call_123");
                assert_eq!(status.as_deref(), Some("success"));
            }
            _ => panic!("Expected Tool message"),
        }
    }

    #[tokio::test]
    async fn test_auto_tool_executor_with_multiple_tool_calls() {
        use crate::core::messages::{Message, MessageContent, ToolCall};

        struct UppercaseTool;

        #[async_trait]
        impl Tool for UppercaseTool {
            fn name(&self) -> &str {
                "uppercase"
            }

            fn description(&self) -> &str {
                "Convert to uppercase"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::String(s) => Ok(s.to_uppercase()),
                    _ => Err(crate::core::Error::Other("Expected string".to_string())),
                }
            }
        }

        struct ReverseTool;

        #[async_trait]
        impl Tool for ReverseTool {
            fn name(&self) -> &str {
                "reverse"
            }

            fn description(&self) -> &str {
                "Reverse string"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::String(s) => Ok(s.chars().rev().collect()),
                    _ => Err(crate::core::Error::Other("Expected string".to_string())),
                }
            }
        }

        let tool_calls = vec![
            ToolCall {
                id: "call_1".to_string(),
                name: "uppercase".to_string(),
                args: serde_json::Value::String("hello".to_string()),
                tool_type: "function".to_string(),
                index: None,
            },
            ToolCall {
                id: "call_2".to_string(),
                name: "reverse".to_string(),
                args: serde_json::Value::String("world".to_string()),
                tool_type: "function".to_string(),
                index: None,
            },
        ];

        let messages = vec![
            Message::human("Transform these strings"),
            Message::AI {
                content: MessageContent::Text("I'll transform them".to_string()),
                tool_calls,
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: Default::default(),
            },
        ];

        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(UppercaseTool), Arc::new(ReverseTool)];

        let result = super::auto_tool_executor(&messages, &tools).await.unwrap();
        assert_eq!(result.len(), 2);

        // Check first tool result
        match &result[0] {
            Message::Tool {
                content,
                tool_call_id,
                ..
            } => {
                assert_eq!(
                    content.as_text(),
                    "=== TOOL OUTPUT FROM 'uppercase' ===\n\nHELLO\n\n=== END TOOL OUTPUT ===\n\nIMPORTANT: Use this tool output in your response."
                );
                assert_eq!(tool_call_id, "call_1");
            }
            _ => panic!("Expected Tool message"),
        }

        // Check second tool result
        match &result[1] {
            Message::Tool {
                content,
                tool_call_id,
                ..
            } => {
                assert_eq!(
                    content.as_text(),
                    "=== TOOL OUTPUT FROM 'reverse' ===\n\ndlrow\n\n=== END TOOL OUTPUT ===\n\nIMPORTANT: Use this tool output in your response."
                );
                assert_eq!(tool_call_id, "call_2");
            }
            _ => panic!("Expected Tool message"),
        }
    }

    #[tokio::test]
    async fn test_auto_tool_executor_tool_not_found() {
        use crate::core::messages::{Message, MessageContent, ToolCall};

        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "nonexistent".to_string(),
            args: serde_json::Value::String("test".to_string()),
            tool_type: "function".to_string(),
            index: None,
        };

        let messages = vec![Message::AI {
            content: MessageContent::Text("Calling tool".to_string()),
            tool_calls: vec![tool_call],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        }];

        let tools: Vec<Arc<dyn Tool>> = vec![];

        let result = super::auto_tool_executor(&messages, &tools).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Tool 'nonexistent' not found"));
    }

    #[tokio::test]
    async fn test_auto_tool_executor_tool_execution_failure() {
        use crate::core::messages::{Message, MessageContent, ToolCall};

        struct FailingTool;

        #[async_trait]
        impl Tool for FailingTool {
            fn name(&self) -> &str {
                "failing"
            }

            fn description(&self) -> &str {
                "Always fails"
            }

            async fn _call(&self, _input: ToolInput) -> crate::core::Result<String> {
                Err(crate::core::Error::Other("Tool failed".to_string()))
            }
        }

        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "failing".to_string(),
            args: serde_json::Value::String("test".to_string()),
            tool_type: "function".to_string(),
            index: None,
        };

        let messages = vec![Message::AI {
            content: MessageContent::Text("Calling tool".to_string()),
            tool_calls: vec![tool_call],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        }];

        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(FailingTool)];

        let result = super::auto_tool_executor(&messages, &tools).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // Error format changed to NodeExecution typed error
        assert!(err_msg.contains("tool:failing") || err_msg.contains("failing"));
    }

    #[tokio::test]
    async fn test_auto_tool_executor_with_structured_args() {
        use crate::core::messages::{Message, MessageContent, ToolCall};

        struct CalculatorTool;

        #[async_trait]
        impl Tool for CalculatorTool {
            fn name(&self) -> &str {
                "calculator"
            }

            fn description(&self) -> &str {
                "Calculate"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::Structured(json) => {
                        let a = json["a"].as_i64().unwrap_or(0);
                        let b = json["b"].as_i64().unwrap_or(0);
                        Ok((a + b).to_string())
                    }
                    _ => Err(crate::core::Error::Other(
                        "Expected structured input".to_string(),
                    )),
                }
            }
        }

        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "calculator".to_string(),
            args: serde_json::json!({"a": 5, "b": 7}),
            tool_type: "function".to_string(),
            index: None,
        };

        let messages = vec![Message::AI {
            content: MessageContent::Text("Calculating".to_string()),
            tool_calls: vec![tool_call],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        }];

        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(CalculatorTool)];

        let result = super::auto_tool_executor(&messages, &tools).await.unwrap();
        assert_eq!(result.len(), 1);

        match &result[0] {
            Message::Tool { content, .. } => {
                assert_eq!(
                    content.as_text(),
                    "=== TOOL OUTPUT FROM 'calculator' ===\n\n12\n\n=== END TOOL OUTPUT ===\n\nIMPORTANT: Use this tool output in your response."
                );
            }
            _ => panic!("Expected Tool message"),
        }
    }

    #[tokio::test]
    async fn test_auto_tool_executor_last_message_not_ai() {
        use crate::core::messages::Message;

        let messages = vec![Message::ai("Response"), Message::human("Follow up")];

        let tools: Vec<Arc<dyn Tool>> = vec![];

        let result = super::auto_tool_executor(&messages, &tools).await.unwrap();
        assert!(result.is_empty());
    }

    // ===== Edge Case Tests =====

    #[tokio::test]
    async fn test_runnable_node_zero_value_state() {
        let runnable = DoubleRunnable;
        let node = RunnableNode::new("double_zero", runnable);

        let state = TestState { value: 0 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 0);
    }

    #[tokio::test]
    async fn test_runnable_node_negative_value_state() {
        let runnable = DoubleRunnable;
        let node = RunnableNode::new("double_negative", runnable);

        let state = TestState { value: -5 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, -10);
    }

    #[tokio::test]
    async fn test_runnable_node_max_value_state() {
        struct IdentityRunnable;

        #[async_trait]
        impl Runnable for IdentityRunnable {
            type Input = TestState;
            type Output = TestState;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Ok(input)
            }
        }

        let node = RunnableNode::new("identity_max", IdentityRunnable);
        let state = TestState { value: i32::MAX };
        let result = node.execute(state.clone()).await.unwrap();
        assert_eq!(result.value, i32::MAX);
    }

    #[tokio::test]
    async fn test_agent_node_zero_input() {
        struct ZeroAgent;

        #[async_trait]
        impl Runnable for ZeroAgent {
            type Input = i32;
            type Output = i32;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Ok(input * 2)
            }
        }

        let agent = ZeroAgent;
        let node = AgentNode::new(
            "zero_agent",
            agent,
            |state: TestState| state.value,
            |_state, output| TestState { value: output },
        );

        let state = TestState { value: 0 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, 0);
    }

    #[tokio::test]
    async fn test_agent_node_negative_input() {
        struct NegAgent;

        #[async_trait]
        impl Runnable for NegAgent {
            type Input = i32;
            type Output = i32;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Ok(input - 10)
            }
        }

        let agent = NegAgent;
        let node = AgentNode::new(
            "neg_agent",
            agent,
            |state: TestState| state.value,
            |_state, output| TestState { value: output },
        );

        let state = TestState { value: -5 };
        let result = node.execute(state).await.unwrap();
        assert_eq!(result.value, -15);
    }

    #[tokio::test]
    async fn test_tool_node_very_long_string_input() {
        struct TruncateTool;

        #[async_trait]
        impl Tool for TruncateTool {
            fn name(&self) -> &str {
                "truncate"
            }

            fn description(&self) -> &str {
                "Truncates to first 10 chars"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::String(s) => Ok(s.chars().take(10).collect()),
                    _ => Err(crate::core::Error::Other("Expected string".to_string())),
                }
            }
        }

        #[derive(Clone)]
        struct SimpleState {
            data: String,
        }

        let tool = TruncateTool;
        let node = ToolNode::new(
            tool,
            |state: SimpleState| ToolInput::String(state.data),
            |mut state, output| {
                state.data = output;
                state
            },
        );

        let long_string = "a".repeat(10000);
        let state = SimpleState { data: long_string };

        let result = node.execute(state).await.unwrap();
        assert_eq!(result.data.len(), 10);
        assert_eq!(result.data, "aaaaaaaaaa");
    }

    #[tokio::test]
    async fn test_tool_node_special_characters_input() {
        struct EchoTool;

        #[async_trait]
        impl Tool for EchoTool {
            fn name(&self) -> &str {
                "echo_special"
            }

            fn description(&self) -> &str {
                "Echoes input"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::String(s) => Ok(s),
                    _ => Err(crate::core::Error::Other("Expected string".to_string())),
                }
            }
        }

        #[derive(Clone)]
        struct SimpleState {
            data: String,
        }

        let tool = EchoTool;
        let node = ToolNode::new(
            tool,
            |state: SimpleState| ToolInput::String(state.data),
            |mut state, output| {
                state.data = output;
                state
            },
        );

        let special = "!@#$%^&*(){}[]|\\:;\"'<>,.?/~`\n\t\r";
        let state = SimpleState {
            data: special.to_string(),
        };

        let result = node.execute(state).await.unwrap();
        assert_eq!(result.data, special);
    }

    #[tokio::test]
    async fn test_tool_node_unicode_input() {
        struct UnicodeTool;

        #[async_trait]
        impl Tool for UnicodeTool {
            fn name(&self) -> &str {
                "unicode"
            }

            fn description(&self) -> &str {
                "Handles unicode"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::String(s) => Ok(format!("Processed: {}", s)),
                    _ => Err(crate::core::Error::Other("Expected string".to_string())),
                }
            }
        }

        #[derive(Clone)]
        struct SimpleState {
            data: String,
        }

        let tool = UnicodeTool;
        let node = ToolNode::new(
            tool,
            |state: SimpleState| ToolInput::String(state.data),
            |mut state, output| {
                state.data = output;
                state
            },
        );

        let unicode = "Hello 世界 🚀 Здравствуй مرحبا";
        let state = SimpleState {
            data: unicode.to_string(),
        };

        let result = node.execute(state).await.unwrap();
        assert_eq!(result.data, format!("Processed: {}", unicode));
    }

    #[test]
    fn test_tools_condition_multiple_tool_calls() {
        use crate::core::messages::{Message, ToolCall};

        let tool_calls = vec![
            ToolCall {
                id: "call_1".to_string(),
                name: "search".to_string(),
                args: serde_json::json!({"query": "test1"}),
                tool_type: "function".to_string(),
                index: None,
            },
            ToolCall {
                id: "call_2".to_string(),
                name: "calculator".to_string(),
                args: serde_json::json!({"a": 5, "b": 7}),
                tool_type: "function".to_string(),
                index: None,
            },
            ToolCall {
                id: "call_3".to_string(),
                name: "weather".to_string(),
                args: serde_json::json!({"location": "SF"}),
                tool_type: "function".to_string(),
                index: None,
            },
        ];

        let messages = vec![Message::AI {
            content: crate::core::messages::MessageContent::Text("Multiple tools".to_string()),
            tool_calls,
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        }];

        let result = tools_condition(&messages);
        assert_eq!(result, "tools");
    }

    #[tokio::test]
    async fn test_auto_tool_executor_empty_string_args() {
        use crate::core::messages::{Message, MessageContent, ToolCall};

        struct EmptyHandlerTool;

        #[async_trait]
        impl Tool for EmptyHandlerTool {
            fn name(&self) -> &str {
                "empty_handler"
            }

            fn description(&self) -> &str {
                "Handles empty strings"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::String(s) => {
                        if s.is_empty() {
                            Ok("Empty input handled".to_string())
                        } else {
                            Ok(format!("Got: {}", s))
                        }
                    }
                    _ => Err(crate::core::Error::Other("Expected string".to_string())),
                }
            }
        }

        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "empty_handler".to_string(),
            args: serde_json::Value::String(String::new()),
            tool_type: "function".to_string(),
            index: None,
        };

        let messages = vec![Message::AI {
            content: MessageContent::Text("Testing empty".to_string()),
            tool_calls: vec![tool_call],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        }];

        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EmptyHandlerTool)];

        let result = super::auto_tool_executor(&messages, &tools).await.unwrap();
        assert_eq!(result.len(), 1);

        match &result[0] {
            Message::Tool { content, .. } => {
                assert!(content.as_text().contains("Empty input handled"));
            }
            _ => panic!("Expected Tool message"),
        }
    }

    #[tokio::test]
    async fn test_auto_tool_executor_structured_empty_object() {
        use crate::core::messages::{Message, MessageContent, ToolCall};

        struct EmptyObjectTool;

        #[async_trait]
        impl Tool for EmptyObjectTool {
            fn name(&self) -> &str {
                "empty_object"
            }

            fn description(&self) -> &str {
                "Handles empty objects"
            }

            async fn _call(&self, input: ToolInput) -> crate::core::Result<String> {
                match input {
                    ToolInput::Structured(json) => {
                        if json.as_object().map(|o| o.is_empty()).unwrap_or(false) {
                            Ok("Empty object".to_string())
                        } else {
                            Ok("Not empty".to_string())
                        }
                    }
                    _ => Err(crate::core::Error::Other(
                        "Expected structured input".to_string(),
                    )),
                }
            }
        }

        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "empty_object".to_string(),
            args: serde_json::json!({}),
            tool_type: "function".to_string(),
            index: None,
        };

        let messages = vec![Message::AI {
            content: MessageContent::Text("Testing empty object".to_string()),
            tool_calls: vec![tool_call],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        }];

        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EmptyObjectTool)];

        let result = super::auto_tool_executor(&messages, &tools).await.unwrap();
        assert_eq!(result.len(), 1);

        match &result[0] {
            Message::Tool { content, .. } => {
                assert!(content.as_text().contains("Empty object"));
            }
            _ => panic!("Expected Tool message"),
        }
    }

    #[tokio::test]
    async fn test_auto_tool_executor_tool_returns_empty_string() {
        use crate::core::messages::{Message, MessageContent, ToolCall};

        struct EmptyReturnTool;

        #[async_trait]
        impl Tool for EmptyReturnTool {
            fn name(&self) -> &str {
                "empty_return"
            }

            fn description(&self) -> &str {
                "Returns empty string"
            }

            async fn _call(&self, _input: ToolInput) -> crate::core::Result<String> {
                Ok(String::new())
            }
        }

        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "empty_return".to_string(),
            args: serde_json::Value::String("test".to_string()),
            tool_type: "function".to_string(),
            index: None,
        };

        let messages = vec![Message::AI {
            content: MessageContent::Text("Testing empty return".to_string()),
            tool_calls: vec![tool_call],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        }];

        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EmptyReturnTool)];

        let result = super::auto_tool_executor(&messages, &tools).await.unwrap();
        assert_eq!(result.len(), 1);

        match &result[0] {
            Message::Tool {
                content, status, ..
            } => {
                // Should still format properly with empty content
                assert!(content
                    .as_text()
                    .contains("=== TOOL OUTPUT FROM 'empty_return' ==="));
                assert!(content.as_text().contains("=== END TOOL OUTPUT ==="));
                assert_eq!(status.as_deref(), Some("success"));
            }
            _ => panic!("Expected Tool message"),
        }
    }

    #[tokio::test]
    async fn test_agent_node_config_preserves_name() {
        struct SimpleAgent;

        #[async_trait]
        impl Runnable for SimpleAgent {
            type Input = i32;
            type Output = i32;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Ok(input)
            }
        }

        let agent = SimpleAgent;
        let config = RunnableConfig::default();
        let node = AgentNode::new(
            "agent_with_config",
            agent,
            |state: TestState| state.value,
            |_state, output| TestState { value: output },
        )
        .with_config(config);

        assert_eq!(node.name(), "agent_with_config");
    }

    #[tokio::test]
    async fn test_runnable_node_with_empty_string_name() {
        let runnable = DoubleRunnable;
        let node = RunnableNode::new("", runnable);
        assert_eq!(node.name(), "");
    }

    #[tokio::test]
    async fn test_agent_node_with_empty_string_name() {
        struct SimpleAgent;

        #[async_trait]
        impl Runnable for SimpleAgent {
            type Input = i32;
            type Output = i32;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> crate::core::Result<Self::Output> {
                Ok(input)
            }
        }

        let agent = SimpleAgent;
        let node = AgentNode::new(
            "",
            agent,
            |state: TestState| state.value,
            |_state, output| TestState { value: output },
        );

        assert_eq!(node.name(), "");
    }

    #[tokio::test]
    async fn test_tool_node_with_special_character_name() {
        struct SpecialNameTool;

        #[async_trait]
        impl Tool for SpecialNameTool {
            fn name(&self) -> &str {
                "tool-with-dashes_and_underscores.123"
            }

            fn description(&self) -> &str {
                "Special name"
            }

            async fn _call(&self, _input: ToolInput) -> crate::core::Result<String> {
                Ok("ok".to_string())
            }
        }

        #[derive(Clone)]
        struct SimpleState {
            data: String,
        }

        let tool = SpecialNameTool;
        let node = ToolNode::new(
            tool,
            |state: SimpleState| ToolInput::String(state.data),
            |mut state, output| {
                state.data = output;
                state
            },
        );

        assert_eq!(node.name(), "tool-with-dashes_and_underscores.123");
    }

    #[test]
    fn test_tools_condition_human_message_only() {
        use crate::core::messages::Message;
        use crate::END;

        let messages = vec![Message::human("Hello")];

        let result = tools_condition(&messages);
        assert_eq!(result, END);
    }

    #[test]
    fn test_tools_condition_system_message_last() {
        use crate::core::messages::Message;
        use crate::END;

        let messages = vec![
            Message::human("Hello"),
            Message::ai("Hi"),
            Message::System {
                content: crate::core::messages::MessageContent::Text("System".to_string()),
                fields: Default::default(),
            },
        ];

        let result = tools_condition(&messages);
        assert_eq!(result, END);
    }

    #[tokio::test]
    async fn test_auto_tool_executor_ai_message_with_empty_tool_calls() {
        use crate::core::messages::{Message, MessageContent};

        let messages = vec![Message::AI {
            content: MessageContent::Text("No tools".to_string()),
            tool_calls: vec![],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        }];

        let tools: Vec<Arc<dyn Tool>> = vec![];

        let result = super::auto_tool_executor(&messages, &tools).await.unwrap();
        assert!(result.is_empty());
    }
}
