// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Prebuilt Agent Patterns
//!
//! This module provides convenience functions for creating common agent patterns.
//! These functions construct complete `StateGraph` instances with standard agent
//! workflows, reducing boilerplate and ensuring best practices.
//!
//! # Available Patterns
//!
//! - **`ReAct` Agent**: Reasoning and Acting agent that uses tools in a loop
//!
//! # Example: `ReAct` Agent
//!
//! ```rust,ignore
//! use dashflow::prebuilt::create_react_agent;
//! use dashflow_openai::ChatOpenAI;
//! use dashflow::core::tools::Tool;
//! use std::sync::Arc;
//!
//! // Create model and tools
//! let model = ChatOpenAI::with_config(Default::default());
//! let tools: Vec<Arc<dyn Tool>> = vec![
//!     Arc::new(SearchTool::new()),
//!     Arc::new(CalculatorTool::new()),
//! ];
//!
//! // Create ReAct agent graph
//! let agent = create_react_agent(model, tools)?;
//!
//! // Use the agent
//! let result = agent.invoke(initial_state).await?;
//! ```

use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::core::tools::Tool;
use crate::integration::{auto_tool_executor, tools_condition};
use crate::{CompiledGraph, Error, Result, StateGraph, END};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Standard message-based state for `ReAct` agents
///
/// This state type is used by `create_react_agent()` and follows the
/// DashFlow convention of maintaining a message history.
///
/// # Fields
///
/// * `messages` - Conversation history (Human, AI, Tool messages)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::prebuilt::AgentState;
/// use dashflow::core::messages::Message;
///
/// let mut state = AgentState {
///     messages: vec![Message::human("What's the weather?")],
/// };
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentState {
    /// The conversation history as a list of messages.
    pub messages: Vec<Message>,
}

impl AgentState {
    /// Create a new agent state with an initial message
    #[must_use]
    pub fn new(initial_message: Message) -> Self {
        Self {
            messages: vec![initial_message],
        }
    }

    /// Create a new agent state with a human message
    #[must_use]
    pub fn with_human_message(content: impl Into<String>) -> Self {
        Self::new(Message::human(content.into()))
    }
}

impl crate::state::MergeableState for AgentState {
    fn merge(&mut self, other: &Self) {
        // Extend messages from parallel branches
        self.messages.extend(other.messages.clone());
    }
}

/// Creates a `ReAct` (Reasoning and Acting) agent graph.
///
/// This is the Rust equivalent of Python DashFlow's create_react_agent.
/// It creates a standard agent workflow that:
///
/// 1. Calls the LLM with bound tools
/// 2. If the LLM requests tool calls, executes them
/// 3. Returns tool results to the LLM
/// 4. Repeats until the LLM provides a final answer (no tool calls)
///
/// The agent uses the standard `AgentState` with a `messages` field for conversation history.
///
/// # Arguments
///
/// * `model` - A `ChatModel` instance (e.g., `ChatOpenAI`, `ChatAnthropic`)
/// * `tools` - Vector of tools the agent can use
///
/// # Returns
///
/// A compiled graph that implements the `ReAct` agent pattern.
///
/// # Errors
///
/// Returns an error if the graph cannot be compiled (rare, indicates internal issue).
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::prebuilt::{create_react_agent, AgentState};
/// use dashflow_openai::ChatOpenAI;
/// use dashflow::core::language_models::ChatModelToolBindingExt;
/// use dashflow::core::messages::Message;
/// use std::sync::Arc;
///
/// // Create model with bound tools
/// let model = ChatOpenAI::with_config(Default::default())
///     .bind_tools(vec![Arc::new(search_tool)], None);
///
/// // Create agent
/// let agent = create_react_agent(model, vec![Arc::new(search_tool)])?;
///
/// // Run agent
/// let initial_state = AgentState::with_human_message("Search for Rust async patterns");
/// let result = agent.invoke(initial_state).await?;
///
/// // Access conversation history
/// for message in result.messages {
///     println!("{}", message.as_text());
/// }
/// ```
///
/// # Python Equivalent
///
/// ```python
/// # Equivalent to Python DashFlow's create_react_agent
/// from dashflow_openai import ChatOpenAI
///
/// model = ChatOpenAI()
/// agent = create_react_agent(model, tools)
/// result = agent.invoke({"messages": [("human", "query")]})
/// ```
pub fn create_react_agent<M>(
    model: M,
    tools: Vec<Arc<dyn Tool>>,
) -> Result<CompiledGraph<AgentState>>
where
    M: ChatModel + Clone + 'static,
{
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Move tools and model for use in closures
    let tools_for_node = tools;
    let model_for_node = model;

    // Define the agent node - calls LLM with current message history
    let agent_node = move |state: AgentState| {
        let model = model_for_node.clone();
        Box::pin(async move {
            // Prepend system message to emphasize using tool results AND providing final answers
            let system_message = Message::system(
                "You are a helpful assistant with access to tools.\n\n\
                 IMPORTANT:\n\
                 1. Tool outputs will appear as tool messages in the conversation.\n\
                 2. If a tool returns output, you MUST incorporate it into your response.\n\
                 3. After using tools, provide a COMPLETE answer to the user.\n\
                 4. Avoid repeating identical tool calls; use existing tool output when available.\n\
                 5. If you already have enough information, provide your final answer."
            );

            let mut messages_with_system = vec![system_message];
            messages_with_system.extend(state.messages.clone());

            let ai_message = if crate::stream::get_stream_writer().is_some() {
                use crate::core::error::Error as CoreError;
                use crate::core::messages::AIMessageChunk;
                use serde_json::json;

                match model.stream(&messages_with_system, None, None, None, None).await {
                    Ok(mut stream) => {
                        let mut merged = AIMessageChunk::new("");

                        while let Some(chunk_result) = stream.next().await {
                            let chunk = chunk_result.map_err(|e| Error::NodeExecution {
                                node: "agent".to_string(),
                                source: Box::new(std::io::Error::other(format!(
                                    "LLM streaming failed: {e}"
                                ))),
                            })?;

                            if let Some(writer) = crate::stream::get_stream_writer() {
                                if !chunk.message.content.is_empty() {
                                    writer.write(json!({
                                        "type": "llm_delta",
                                        "delta": &chunk.message.content,
                                    }));
                                }
                                if !chunk.message.tool_calls.is_empty() {
                                    writer.write(json!({
                                        "type": "tool_calls_delta",
                                        "tool_calls": &chunk.message.tool_calls,
                                    }));
                                }
                            }

                            merged = merged.merge(chunk.message);
                        }

                        merged.to_message().into()
                    }
                    Err(CoreError::NotImplemented(_)) => {
                        let result = model
                            .generate(&messages_with_system, None, None, None, None)
                            .await
                            .map_err(|e| Error::NodeExecution {
                                node: "agent".to_string(),
                                source: Box::new(std::io::Error::other(format!(
                                    "LLM generation failed: {e}"
                                ))),
                            })?;

                        result
                            .generations
                            .first()
                            .ok_or_else(|| Error::NodeExecution {
                                node: "agent".to_string(),
                                source: Box::new(std::io::Error::other(
                                    "No response generated from LLM",
                                )),
                            })?
                            .message
                            .clone()
                    }
                    Err(e) => {
                        return Err(Error::NodeExecution {
                            node: "agent".to_string(),
                            source: Box::new(std::io::Error::other(format!(
                                "LLM stream setup failed: {e}"
                            ))),
                        });
                    }
                }
            } else {
                // Call LLM with message history (including system prompt)
                let result = model
                    .generate(&messages_with_system, None, None, None, None)
                    .await
                    .map_err(|e| Error::NodeExecution {
                        node: "agent".to_string(),
                        source: Box::new(std::io::Error::other(format!(
                            "LLM generation failed: {e}"
                        ))),
                    })?;

                // Extract AI message and add to state
                result
                    .generations
                    .first()
                    .ok_or_else(|| Error::NodeExecution {
                        node: "agent".to_string(),
                        source: Box::new(std::io::Error::other("No response generated from LLM")),
                    })?
                    .message
                    .clone()
            };
            let mut new_state = state;
            new_state.messages.push(ai_message);

            Ok(new_state)
        })
            as std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<AgentState>> + Send + 'static>,
            >
    };

    // Define the tools node - executes tool calls from last AI message
    let tools_node = move |state: AgentState| {
        let tools = tools_for_node.clone();
        Box::pin(async move {
            // Execute tools and get response messages
            let tool_messages = auto_tool_executor(&state.messages, &tools)
                .await
                .map_err(|e| Error::NodeExecution {
                    node: "tools".to_string(),
                    source: Box::new(std::io::Error::other(format!("Tool execution failed: {e}"))),
                })?;

            // Append tool messages to state
            let mut new_state = state;
            new_state.messages.extend(tool_messages);

            Ok(new_state)
        })
            as std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<AgentState>> + Send + 'static>,
            >
    };

    // Add nodes to graph
    graph.add_node_from_fn("agent", agent_node);
    graph.add_node_from_fn("tools", tools_node);

    // Set entry point
    graph.set_entry_point("agent");

    // Add conditional routing from agent
    // If AI message has tool_calls -> route to "tools"
    // Otherwise -> route to END
    let mut routes = HashMap::new();
    routes.insert("tools".to_string(), "tools".to_string());
    routes.insert(END.to_string(), END.to_string());

    graph.add_conditional_edges(
        "agent",
        |state: &AgentState| tools_condition(&state.messages).to_string(),
        routes,
    );

    // After tools execute, always go back to agent
    graph.add_edge("tools", "agent");

    // Compile the graph
    graph.compile()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_state_new() {
        let message = Message::human("Hello");
        let state = AgentState::new(message.clone());

        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].as_text(), "Hello");
    }

    #[tokio::test]
    async fn test_agent_state_with_human_message() {
        let state = AgentState::with_human_message("Test message");

        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].as_text(), "Test message");
        assert!(matches!(state.messages[0], Message::Human { .. }));
    }

    #[tokio::test]
    async fn test_agent_state_clone() {
        let state1 = AgentState::with_human_message("Original");
        let state2 = state1.clone();

        assert_eq!(state1.messages.len(), state2.messages.len());
        assert_eq!(state1.messages[0].as_text(), state2.messages[0].as_text());
    }

    #[tokio::test]
    async fn test_agent_state_messages_accumulate() {
        // Test that messages accumulate in state across turns
        let mut state = AgentState::with_human_message("First message");

        assert_eq!(state.messages.len(), 1);

        state.messages.push(Message::ai("First response"));
        assert_eq!(state.messages.len(), 2);

        state.messages.push(Message::human("Second message"));
        assert_eq!(state.messages.len(), 3);

        // Verify all messages are preserved
        assert_eq!(state.messages[0].as_text(), "First message");
        assert_eq!(state.messages[1].as_text(), "First response");
        assert_eq!(state.messages[2].as_text(), "Second message");
    }

    #[tokio::test]
    async fn test_agent_state_serialization() {
        // Test that AgentState can be serialized/deserialized
        let original = AgentState::with_human_message("Test message");

        let json = serde_json::to_string(&original).expect("Failed to serialize");
        let deserialized: AgentState = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(original.messages.len(), deserialized.messages.len());
        assert_eq!(
            original.messages[0].as_text(),
            deserialized.messages[0].as_text()
        );
    }

    #[tokio::test]
    async fn test_agent_state_debug_format() {
        // Test that AgentState implements Debug
        let state = AgentState::with_human_message("Debug test");
        let debug_str = format!("{:?}", state);

        assert!(debug_str.contains("AgentState"));
        assert!(debug_str.contains("messages"));
    }

    // NOTE: Full integration tests for create_react_agent() are in:
    // - crates/dashflow-standard-tests/tests/complete_eval_loop.rs (multi-turn with DashFlow Streaming)
    // - Integration tests verify the actual ReAct pattern with real/mock LLMs
    // Unit tests here focus on AgentState and basic create_react_agent() functionality

    // ============================================================================
    // Mock ChatModel for Unit Testing create_react_agent()
    // ============================================================================

    use crate::core::callbacks::CallbackManager;
    use crate::core::error::Result as CoreResult;
    use crate::core::language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    };
    use crate::core::messages::BaseMessage;
    use crate::core::messages::ToolCall;
    use crate::core::tools::ToolInput;
    use async_trait::async_trait;
    use futures::stream;
    use futures::Stream;
    use futures::StreamExt;
    use serde_json::json;
    use std::pin::Pin;

    /// Mock ChatModel that can return tool calls or final answers
    #[derive(Clone)]
    struct MockChatModelWithTools {
        /// Predefined responses to return in sequence
        responses: Arc<Vec<MockResponse>>,
        /// Track how many times the model was called
        call_count: Arc<std::sync::Mutex<usize>>,
    }

    #[derive(Clone, Debug)]
    enum MockResponse {
        /// Return an AI message with tool calls
        WithToolCalls(Vec<ToolCall>),
        /// Return a final answer (no tool calls)
        FinalAnswer(String),
    }

    impl MockChatModelWithTools {
        fn new(responses: Vec<MockResponse>) -> Self {
            Self {
                responses: Arc::new(responses),
                call_count: Arc::new(std::sync::Mutex::new(0)),
            }
        }

        fn call_count(&self) -> usize {
            *self.call_count.lock().unwrap()
        }
    }

    #[async_trait]
    impl ChatModel for MockChatModelWithTools {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&CallbackManager>,
        ) -> crate::core::error::Result<ChatResult> {
            let mut count = self.call_count.lock().unwrap();
            let idx = *count % self.responses.len();
            *count += 1;

            let response = &self.responses[idx];
            match response {
                MockResponse::WithToolCalls(tool_calls) => Ok(ChatResult {
                    generations: vec![ChatGeneration {
                        message: Message::AI {
                            content: "I'll use the search tool to find information.".into(),
                            tool_calls: tool_calls.clone(),
                            invalid_tool_calls: Vec::new(),
                            usage_metadata: None,
                            fields: Default::default(),
                        },
                        generation_info: None,
                    }],
                    llm_output: None,
                }),
                MockResponse::FinalAnswer(answer) => Ok(ChatResult {
                    generations: vec![ChatGeneration {
                        message: Message::ai(answer.clone()),
                        generation_info: None,
                    }],
                    llm_output: None,
                }),
            }
        }

        fn llm_type(&self) -> &str {
            "mock-model-with-tools"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    /// Mock tool for testing
    #[derive(Clone)]
    struct MockSearchTool;

    #[async_trait]
    impl Tool for MockSearchTool {
        fn name(&self) -> &str {
            "search"
        }

        fn description(&self) -> &str {
            "Search for information"
        }

        async fn _call(&self, _input: ToolInput) -> CoreResult<String> {
            Ok("Mock search result: Found documentation about Rust.".to_string())
        }
    }

    // ============================================================================
    // Unit Tests for create_react_agent()
    // ============================================================================

    #[tokio::test]
    async fn test_create_react_agent_compiles() {
        // Test that create_react_agent() successfully compiles a graph
        let model =
            MockChatModelWithTools::new(vec![MockResponse::FinalAnswer("Hello!".to_string())]);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MockSearchTool)];

        let agent = create_react_agent(model, tools);
        assert!(agent.is_ok(), "Agent should compile successfully");
    }

    #[tokio::test]
    async fn test_react_agent_simple_query_no_tools() {
        // Test agent with a simple query that doesn't require tools
        let model = MockChatModelWithTools::new(vec![MockResponse::FinalAnswer(
            "The answer is 42.".to_string(),
        )]);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MockSearchTool)];

        let agent = create_react_agent(model.clone(), tools).expect("Agent should compile");

        let initial_state = AgentState::with_human_message("What is the answer?");
        let result = agent
            .invoke(initial_state)
            .await
            .expect("Agent should execute successfully");

        // Should have: [Human, System (added by agent_node), AI]
        // Note: System message is prepended by agent_node but not persisted to state
        assert_eq!(
            result.final_state.messages.len(),
            2,
            "Should have human + AI messages"
        );
        assert!(result.final_state.messages[0].is_human());
        assert!(result.final_state.messages[1].is_ai());
        assert_eq!(
            result.final_state.messages[1].as_text(),
            "The answer is 42."
        );

        // Model should be called once (no tool loop)
        assert_eq!(model.call_count(), 1);
    }

    #[tokio::test]
    async fn test_react_agent_stream_mode_custom_emits_llm_deltas() {
        use crate::core::messages::AIMessageChunk;
        use crate::stream::{StreamEvent, StreamMode};

        #[derive(Clone)]
        struct StreamingMockModel;

        #[async_trait]
        impl ChatModel for StreamingMockModel {
            async fn _generate(
                &self,
                _messages: &[BaseMessage],
                _stop: Option<&[String]>,
                _tools: Option<&[ToolDefinition]>,
                _tool_choice: Option<&ToolChoice>,
                _run_manager: Option<&CallbackManager>,
            ) -> crate::core::error::Result<ChatResult> {
                Ok(ChatResult {
                    generations: vec![ChatGeneration {
                        message: Message::ai("Hello".to_string()),
                        generation_info: None,
                    }],
                    llm_output: None,
                })
            }

            async fn _stream(
                &self,
                _messages: &[BaseMessage],
                _stop: Option<&[String]>,
                _tools: Option<&[ToolDefinition]>,
                _tool_choice: Option<&ToolChoice>,
                _run_manager: Option<&CallbackManager>,
            ) -> crate::core::error::Result<
                Pin<Box<dyn Stream<Item = crate::core::error::Result<ChatGenerationChunk>> + Send>>,
            > {
                let chunks = vec![
                    Ok(ChatGenerationChunk::new(AIMessageChunk::new("Hel"))),
                    Ok(ChatGenerationChunk::new(AIMessageChunk::new("lo"))),
                ];
                Ok(Box::pin(stream::iter(chunks)))
            }

            fn llm_type(&self) -> &str {
                "streaming-mock"
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let model = StreamingMockModel;
        let tools: Vec<Arc<dyn Tool>> = vec![];
        let agent = create_react_agent(model, tools).expect("Agent should compile");

        let initial_state = AgentState::with_human_message("Say hello");
        let mut s = Box::pin(agent.stream(initial_state, StreamMode::Custom));

        let mut deltas = String::new();
        let mut done: Option<AgentState> = None;

        while let Some(event) = s.next().await {
            match event.expect("stream event should succeed") {
                StreamEvent::Custom { data, .. } => {
                    if data.get("type").and_then(|v| v.as_str()) == Some("llm_delta") {
                        if let Some(delta) = data.get("delta").and_then(|v| v.as_str()) {
                            deltas.push_str(delta);
                        }
                    }
                }
                StreamEvent::Done { state, .. } => {
                    done = Some(state);
                    break;
                }
                _ => {}
            }
        }

        assert_eq!(deltas, "Hello");
        let done = done.expect("expected Done event");
        assert_eq!(done.messages.len(), 2);
        assert_eq!(done.messages[1].as_text(), "Hello");
    }

    #[tokio::test]
    async fn test_react_agent_single_tool_call() {
        // Test agent that makes one tool call then provides final answer
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "search".to_string(),
            args: json!({"query": "Rust async"}),
            tool_type: "tool_call".to_string(),
            index: None,
        };

        let model = MockChatModelWithTools::new(vec![
            MockResponse::WithToolCalls(vec![tool_call]),
            MockResponse::FinalAnswer(
                "Based on the search results, Rust async is awesome.".to_string(),
            ),
        ]);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MockSearchTool)];

        let agent = create_react_agent(model.clone(), tools).expect("Agent should compile");

        let initial_state = AgentState::with_human_message("Tell me about Rust async");
        let result = agent
            .invoke(initial_state)
            .await
            .expect("Agent should execute successfully");

        // Should have: [Human, AI with tool_call, Tool response, AI final]
        assert_eq!(
            result.final_state.messages.len(),
            4,
            "Should have human + AI + tool + AI messages"
        );
        assert!(result.final_state.messages[0].is_human());
        assert!(result.final_state.messages[1].is_ai());

        // Check tool call in second message
        if let Message::AI { tool_calls, .. } = &result.final_state.messages[1] {
            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].name, "search");
        } else {
            panic!("Second message should be AI with tool calls");
        }

        // Third message should be tool response
        assert_eq!(result.final_state.messages[2].message_type(), "tool");
        assert!(result.final_state.messages[2]
            .as_text()
            .contains("Mock search result"));

        // Fourth message should be final AI answer
        assert!(result.final_state.messages[3].is_ai());
        assert!(result.final_state.messages[3]
            .as_text()
            .contains("Based on the search results"));

        // Model should be called twice (once for tool call, once for final answer)
        assert_eq!(model.call_count(), 2);
    }

    #[tokio::test]
    async fn test_react_agent_multiple_tool_calls() {
        // Test agent that makes multiple tool calls in sequence
        let tool_call_1 = ToolCall {
            id: "call_1".to_string(),
            name: "search".to_string(),
            args: json!({"query": "Rust"}),
            tool_type: "tool_call".to_string(),
            index: None,
        };

        let tool_call_2 = ToolCall {
            id: "call_2".to_string(),
            name: "search".to_string(),
            args: json!({"query": "async patterns"}),
            tool_type: "tool_call".to_string(),
            index: None,
        };

        let model = MockChatModelWithTools::new(vec![
            MockResponse::WithToolCalls(vec![tool_call_1]),
            MockResponse::WithToolCalls(vec![tool_call_2]),
            MockResponse::FinalAnswer("Final comprehensive answer.".to_string()),
        ]);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MockSearchTool)];

        let agent = create_react_agent(model.clone(), tools).expect("Agent should compile");

        let initial_state = AgentState::with_human_message("Research Rust async patterns");
        let result = agent
            .invoke(initial_state)
            .await
            .expect("Agent should execute successfully");

        // Should have: [Human, AI1 + tool, Tool1, AI2 + tool, Tool2, AI final]
        assert_eq!(result.final_state.messages.len(), 6);
        assert!(result.final_state.messages[0].is_human());
        assert!(result.final_state.messages[5].is_ai());
        assert!(result.final_state.messages[5]
            .as_text()
            .contains("comprehensive"));

        // Model should be called 3 times
        assert_eq!(model.call_count(), 3);
    }

    #[tokio::test]
    async fn test_react_agent_preserves_conversation_history() {
        // Test that agent preserves all messages in conversation history
        let model = MockChatModelWithTools::new(vec![MockResponse::FinalAnswer(
            "First response".to_string(),
        )]);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MockSearchTool)];

        let agent = create_react_agent(model, tools).expect("Agent should compile");

        // Start with some existing conversation history
        let mut state = AgentState::new(Message::human("First question"));
        state.messages.push(Message::ai("First answer"));
        state.messages.push(Message::human("Second question"));

        let result = agent
            .invoke(state)
            .await
            .expect("Agent should execute successfully");

        // Should preserve all previous messages + add new AI response
        assert_eq!(result.final_state.messages.len(), 4);
        assert_eq!(result.final_state.messages[0].as_text(), "First question");
        assert_eq!(result.final_state.messages[1].as_text(), "First answer");
        assert_eq!(result.final_state.messages[2].as_text(), "Second question");
        assert_eq!(result.final_state.messages[3].as_text(), "First response");
    }

    #[tokio::test]
    async fn test_react_agent_with_empty_tools() {
        // Test that agent works even with no tools (though not very useful)
        let model = MockChatModelWithTools::new(vec![MockResponse::FinalAnswer(
            "I don't have any tools.".to_string(),
        )]);
        let tools: Vec<Arc<dyn Tool>> = vec![];

        let agent = create_react_agent(model, tools);
        assert!(agent.is_ok(), "Agent should compile with empty tools");

        let agent = agent.unwrap();
        let initial_state = AgentState::with_human_message("Hello");
        let result = agent.invoke(initial_state).await;
        assert!(result.is_ok(), "Agent should execute with no tools");
    }

    #[tokio::test]
    async fn test_react_agent_model_error_propagation() {
        // Test that errors from the LLM are properly propagated
        #[derive(Clone)]
        struct FailingMockModel;

        #[async_trait]
        impl ChatModel for FailingMockModel {
            async fn _generate(
                &self,
                _messages: &[BaseMessage],
                _stop: Option<&[String]>,
                _tools: Option<&[ToolDefinition]>,
                _tool_choice: Option<&ToolChoice>,
                _run_manager: Option<&CallbackManager>,
            ) -> crate::core::error::Result<ChatResult> {
                Err(crate::core::error::Error::api("LLM API error"))
            }

            fn llm_type(&self) -> &str {
                "failing-mock"
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let model = FailingMockModel;
        let tools: Vec<Arc<dyn Tool>> = vec![];

        let agent = create_react_agent(model, tools);
        assert!(
            agent.is_ok(),
            "Agent should compile even with failing model"
        );

        let agent = agent.unwrap();
        let initial_state = AgentState::with_human_message("This will fail");
        let result = agent.invoke(initial_state).await;

        // Error should be propagated from the model
        assert!(result.is_err(), "Should propagate LLM error");
    }

    #[tokio::test]
    async fn test_react_agent_tool_error_handling() {
        // Test that errors from tools are properly handled
        #[derive(Clone)]
        struct FailingTool;

        #[async_trait]
        impl Tool for FailingTool {
            fn name(&self) -> &str {
                "failing_tool"
            }

            fn description(&self) -> &str {
                "A tool that always fails"
            }

            async fn _call(&self, _input: ToolInput) -> CoreResult<String> {
                Err(crate::core::error::Error::tool_error(
                    "Tool execution failed",
                ))
            }
        }

        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "failing_tool".to_string(),
            args: json!({}),
            tool_type: "tool_call".to_string(),
            index: None,
        };

        let model = MockChatModelWithTools::new(vec![
            MockResponse::WithToolCalls(vec![tool_call]),
            MockResponse::FinalAnswer("Handled the error".to_string()),
        ]);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(FailingTool)];

        let agent = create_react_agent(model, tools).expect("Agent should compile");

        let initial_state = AgentState::with_human_message("Test tool error");
        let result = agent.invoke(initial_state).await;

        // Error should be propagated from tool execution
        assert!(result.is_err(), "Should propagate tool execution error");
    }

    #[tokio::test]
    async fn test_react_agent_concurrent_tool_calls() {
        // Test agent handling multiple tool calls in a single AI response
        let tool_call_1 = ToolCall {
            id: "call_1".to_string(),
            name: "search".to_string(),
            args: json!({"query": "Rust"}),
            tool_type: "tool_call".to_string(),
            index: Some(0),
        };

        let tool_call_2 = ToolCall {
            id: "call_2".to_string(),
            name: "search".to_string(),
            args: json!({"query": "Python"}),
            tool_type: "tool_call".to_string(),
            index: Some(1),
        };

        let model = MockChatModelWithTools::new(vec![
            MockResponse::WithToolCalls(vec![tool_call_1, tool_call_2]),
            MockResponse::FinalAnswer(
                "Compared Rust and Python based on search results.".to_string(),
            ),
        ]);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MockSearchTool)];

        let agent = create_react_agent(model.clone(), tools).expect("Agent should compile");

        let initial_state = AgentState::with_human_message("Compare Rust and Python");
        let result = agent
            .invoke(initial_state)
            .await
            .expect("Agent should handle concurrent tool calls");

        // Should have: [Human, AI + 2 tool calls, Tool1, Tool2, AI final]
        assert_eq!(result.final_state.messages.len(), 5);

        // First message is human
        assert!(result.final_state.messages[0].is_human());

        // Second message is AI with tool calls
        if let Message::AI { tool_calls, .. } = &result.final_state.messages[1] {
            assert_eq!(tool_calls.len(), 2);
            assert_eq!(tool_calls[0].name, "search");
            assert_eq!(tool_calls[1].name, "search");
        } else {
            panic!("Second message should be AI with 2 tool calls");
        }

        // Third and fourth messages are tool responses
        assert_eq!(result.final_state.messages[2].message_type(), "tool");
        assert_eq!(result.final_state.messages[3].message_type(), "tool");

        // Fifth message is final answer
        assert!(result.final_state.messages[4].is_ai());
        assert!(result.final_state.messages[4]
            .as_text()
            .contains("Compared"));

        // Model should be called twice
        assert_eq!(model.call_count(), 2);
    }

    #[tokio::test]
    async fn test_react_agent_tool_not_found() {
        // Test agent behavior when LLM calls a tool that doesn't exist
        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "nonexistent_tool".to_string(),
            args: json!({}),
            tool_type: "tool_call".to_string(),
            index: None,
        };

        let model = MockChatModelWithTools::new(vec![
            MockResponse::WithToolCalls(vec![tool_call]),
            MockResponse::FinalAnswer("Final answer".to_string()),
        ]);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MockSearchTool)];

        let agent = create_react_agent(model, tools).expect("Agent should compile");

        let initial_state = AgentState::with_human_message("Call nonexistent tool");
        let result = agent.invoke(initial_state).await;

        // Should fail with tool not found error
        assert!(result.is_err(), "Should error when tool is not found");
    }

    #[tokio::test]
    async fn test_react_agent_empty_initial_messages() {
        // Test agent with an empty message list (edge case)
        let model =
            MockChatModelWithTools::new(vec![MockResponse::FinalAnswer("Response".to_string())]);
        let tools: Vec<Arc<dyn Tool>> = vec![];

        let agent = create_react_agent(model, tools).expect("Agent should compile");

        // Create state with empty messages (unusual but valid)
        let initial_state = AgentState { messages: vec![] };

        let result = agent.invoke(initial_state).await;

        // Agent should handle empty messages gracefully
        assert!(result.is_ok(), "Agent should handle empty messages");
        let final_state = result.unwrap().final_state;

        // Should have exactly one message (the AI response)
        assert_eq!(final_state.messages.len(), 1);
        assert!(final_state.messages[0].is_ai());
    }

    #[tokio::test]
    async fn test_react_agent_long_conversation_chain() {
        // Test agent maintains coherence over many iterations
        let model = MockChatModelWithTools::new(vec![
            MockResponse::WithToolCalls(vec![ToolCall {
                id: "call_1".to_string(),
                name: "search".to_string(),
                args: json!({"query": "Step 1"}),
                tool_type: "tool_call".to_string(),
                index: None,
            }]),
            MockResponse::WithToolCalls(vec![ToolCall {
                id: "call_2".to_string(),
                name: "search".to_string(),
                args: json!({"query": "Step 2"}),
                tool_type: "tool_call".to_string(),
                index: None,
            }]),
            MockResponse::WithToolCalls(vec![ToolCall {
                id: "call_3".to_string(),
                name: "search".to_string(),
                args: json!({"query": "Step 3"}),
                tool_type: "tool_call".to_string(),
                index: None,
            }]),
            MockResponse::FinalAnswer("Completed all 3 steps successfully.".to_string()),
        ]);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MockSearchTool)];

        let agent = create_react_agent(model.clone(), tools).expect("Agent should compile");

        let initial_state = AgentState::with_human_message("Execute a 3-step plan");
        let result = agent
            .invoke(initial_state)
            .await
            .expect("Agent should handle long conversation chains");

        // Should have: [Human, AI1+tool, Tool1, AI2+tool, Tool2, AI3+tool, Tool3, AI final]
        assert_eq!(result.final_state.messages.len(), 8);

        // Verify message sequence
        assert!(result.final_state.messages[0].is_human());
        assert!(result.final_state.messages[1].is_ai());
        assert_eq!(result.final_state.messages[2].message_type(), "tool");
        assert!(result.final_state.messages[3].is_ai());
        assert_eq!(result.final_state.messages[4].message_type(), "tool");
        assert!(result.final_state.messages[5].is_ai());
        assert_eq!(result.final_state.messages[6].message_type(), "tool");
        assert!(result.final_state.messages[7].is_ai());

        // Model should be called 4 times (3 tool calls + 1 final answer)
        assert_eq!(model.call_count(), 4);
    }

    #[tokio::test]
    async fn test_agent_state_clone_independence() {
        // Test that cloned states are independent
        let original = AgentState::with_human_message("Original");
        let mut cloned = original.clone();

        cloned.messages.push(Message::ai("Cloned addition"));

        // Original should be unchanged
        assert_eq!(original.messages.len(), 1);
        assert_eq!(cloned.messages.len(), 2);

        assert_eq!(original.messages[0].as_text(), "Original");
        assert_eq!(cloned.messages[0].as_text(), "Original");
        assert_eq!(cloned.messages[1].as_text(), "Cloned addition");
    }

    #[tokio::test]
    async fn test_react_agent_with_system_message() {
        // Test agent behavior with system message in initial state
        let model = MockChatModelWithTools::new(vec![MockResponse::FinalAnswer(
            "Following system instructions".to_string(),
        )]);
        let tools: Vec<Arc<dyn Tool>> = vec![];

        let agent = create_react_agent(model, tools).expect("Agent should compile");

        // Start with system message + human message
        let mut initial_state = AgentState::new(Message::system("You are a helpful assistant."));
        initial_state.messages.push(Message::human("What are you?"));

        let result = agent
            .invoke(initial_state)
            .await
            .expect("Agent should handle system messages");

        // Should preserve system message and add AI response
        assert_eq!(result.final_state.messages.len(), 3);
        assert_eq!(result.final_state.messages[0].message_type(), "system");
        assert!(result.final_state.messages[1].is_human());
        assert!(result.final_state.messages[2].is_ai());
    }
}
