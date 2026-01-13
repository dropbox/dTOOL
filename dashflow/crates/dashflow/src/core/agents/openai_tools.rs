use crate::core::error::Result;

use super::{Agent, AgentAction, AgentDecision, AgentFinish, AgentStep};

/// `OpenAI` Tools Agent that uses `OpenAI`'s modern tools calling API
///
/// This agent is specifically designed for `OpenAI`'s newer "tools" API format
/// (as opposed to the older "functions" API). It formats tools with OpenAI-specific
/// schema and parses the model's `tool_calls` array responses.
///
/// # Pattern
///
/// 1. Agent receives input and intermediate steps
/// 2. Formats tools as `OpenAI` tool definitions (with "type": "function" wrapper)
/// 3. LLM responds with either:
///    - `tool_calls` array in message → AgentAction(s)
///    - Regular message content → `AgentFinish`
/// 4. Tool results are formatted as `ToolMessage` for scratchpad
///
/// # Comparison to Other Agents
///
/// **`OpenAIToolsAgent` (this)**:
/// - Uses `OpenAI`'s modern `tools` parameter and `tool_calls` array
/// - Uses `Message::Tool` for scratchpad
/// - Supports multiple tool calls in one response
/// - Recommended for new `OpenAI` integrations
///
/// **`OpenAIFunctionsAgent`**:
/// - Uses `OpenAI`'s older `function_call` format (single call only)
/// - Uses `Message::Function` for scratchpad
/// - Legacy compatibility for older `OpenAI` models
///
/// **`ToolCallingAgent`**:
/// - Generic tool calling agent (works with multiple providers)
/// - Similar to `OpenAIToolsAgent` but provider-agnostic
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::agents::OpenAIToolsAgent;
/// use dashflow::core::language_models::ChatModel;
/// use std::sync::Arc;
///
/// async fn example(llm: Arc<dyn ChatModel>) {
///     let tools = vec![/* tools */];
///
///     let agent = OpenAIToolsAgent::new(
///         llm,
///         tools,
///         "You are a helpful assistant with access to tools."
///     );
///
///     // Use with AgentExecutor
/// }
/// ```
pub struct OpenAIToolsAgent {
    /// The chat model to use for reasoning (should support `OpenAI` tools)
    chat_model: std::sync::Arc<dyn crate::core::language_models::ChatModel>,

    /// Tools available to the agent (used in test methods)
    #[allow(dead_code)] // Test: Accessed by tools_to_openai_tools() in test code
    tools: Vec<std::sync::Arc<dyn crate::core::tools::Tool>>,

    /// Tool definitions for passing to language model
    tool_definitions: Vec<crate::core::language_models::ToolDefinition>,

    /// System prompt that guides the agent's behavior
    system_prompt: String,
}

impl OpenAIToolsAgent {
    /// Create a new `OpenAI` Tools agent
    ///
    /// # Arguments
    ///
    /// * `chat_model` - Chat model that supports `OpenAI` tool calling
    /// * `tools` - List of tools the agent can use
    /// * `system_prompt` - Instructions for the agent
    pub fn new(
        chat_model: std::sync::Arc<dyn crate::core::language_models::ChatModel>,
        tools: Vec<std::sync::Arc<dyn crate::core::tools::Tool>>,
        system_prompt: impl Into<String>,
    ) -> Self {
        let tool_definitions = crate::core::tools::tools_to_definitions(&tools);
        Self {
            chat_model,
            tools,
            tool_definitions,
            system_prompt: system_prompt.into(),
        }
    }

    /// Convert tools to OpenAI tool definitions
    ///
    /// Returns a JSON array of tool definitions in OpenAI tools format:
    /// ```json
    /// {
    ///   "type": "function",
    ///   "function": {
    ///     "name": "tool_name",
    ///     "description": "What the tool does",
    ///     "parameters": { /* JSON Schema */ }
    ///   }
    /// }
    /// ```
    #[cfg(test)]
    pub(super) fn tools_to_openai_tools(&self) -> Vec<serde_json::Value> {
        self.tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.args_schema()
                    }
                })
            })
            .collect()
    }

    /// Convert intermediate steps into `ToolMessage` format
    ///
    /// Each step becomes:
    /// 1. `AIMessage` with `tool_calls` array
    /// 2. `ToolMessage` with the observation result and `tool_call_id`
    pub(super) fn steps_to_messages(
        &self,
        steps: &[AgentStep],
    ) -> Vec<crate::core::messages::BaseMessage> {
        use crate::core::messages::{BaseMessage, Message, MessageContent, ToolCall};

        let mut messages = Vec::new();

        for step in steps {
            // Add AIMessage with tool_call
            let tool_call = ToolCall {
                id: format!("call_{}", uuid::Uuid::new_v4()),
                name: step.action.tool.clone(),
                args: match &step.action.tool_input {
                    crate::core::tools::ToolInput::String(s) => {
                        // Simple string input - wrap in a {"__arg1": value} structure
                        serde_json::json!({"__arg1": s})
                    }
                    crate::core::tools::ToolInput::Structured(v) => {
                        // Already structured - use as-is
                        v.clone()
                    }
                },
                tool_type: "function".to_string(),
                index: None,
            };

            // Save tool_call.id before moving tool_call
            let tool_call_id = tool_call.id.clone();

            messages.push(BaseMessage::from(Message::AI {
                content: MessageContent::Text(step.action.log.clone()),
                tool_calls: vec![tool_call], // Move tool_call (no clone)
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: crate::core::messages::BaseMessageFields::default(),
            }));

            // Add ToolMessage with observation using saved id
            messages.push(BaseMessage::from(Message::tool(
                step.observation.clone(),
                tool_call_id,
            )));
        }

        messages
    }
}

#[async_trait::async_trait]
impl Agent for OpenAIToolsAgent {
    async fn plan(&self, input: &str, intermediate_steps: &[AgentStep]) -> Result<AgentDecision> {
        use crate::core::messages::{BaseMessage, Message};

        // Build message list
        let mut messages = vec![BaseMessage::from(Message::system(
            self.system_prompt.clone(),
        ))];

        // Add intermediate steps as tool messages
        messages.extend(self.steps_to_messages(intermediate_steps));

        // Add current input
        messages.push(BaseMessage::from(Message::human(input)));

        // Call chat model with tool definitions
        let result = self
            .chat_model
            .generate(
                &messages,
                None,
                Some(&self.tool_definitions),
                Some(&crate::core::language_models::ToolChoice::Auto),
                None,
            )
            .await?;

        // Get the first generation (should always exist)
        let generation = result.generations.first().ok_or_else(|| {
            crate::core::error::Error::Agent("No generations returned from chat model".to_string())
        })?;

        // Check if the message has tool calls
        if let Message::AI {
            tool_calls,
            content,
            fields,
            ..
        } = &generation.message
        {
            // First check the tool_calls field (modern format)
            if !tool_calls.is_empty() {
                // Model wants to use a tool - return the first tool call as an action
                let tool_call = &tool_calls[0];

                // Extract tool input, handling __arg1 special case
                let _tool_input = tool_call.args.clone();
                let tool_input = if let Some(arg1) = _tool_input.get("__arg1") {
                    // __arg1 special case for old-style single string argument tools
                    if let Some(s) = arg1.as_str() {
                        crate::core::tools::ToolInput::String(s.to_string())
                    } else {
                        crate::core::tools::ToolInput::Structured(_tool_input)
                    }
                } else if _tool_input.is_object() || _tool_input.is_array() {
                    crate::core::tools::ToolInput::Structured(_tool_input)
                } else {
                    // If not structured, convert to string
                    crate::core::tools::ToolInput::String(_tool_input.to_string())
                };

                let log = if content.as_text().is_empty() {
                    format!("Calling tool: {} with {:?}", tool_call.name, tool_input)
                } else {
                    content.as_text()
                };

                return Ok(AgentDecision::Action(AgentAction {
                    tool: tool_call.name.clone(),
                    tool_input,
                    log,
                }));
            }

            // Fallback: check additional_kwargs for tool_calls (some implementations put it there)
            if let Some(tool_calls_value) = fields.additional_kwargs.get("tool_calls") {
                if let Some(tool_calls_array) = tool_calls_value.as_array() {
                    if !tool_calls_array.is_empty() {
                        // Parse the first tool call from additional_kwargs
                        let tool_call_obj = &tool_calls_array[0];
                        if let Some(function) = tool_call_obj.get("function") {
                            let function_name = function
                                .get("name")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    crate::core::error::Error::Agent(
                                        "tool_call missing function.name field".to_string(),
                                    )
                                })?
                                .to_string();

                            let arguments_str = function
                                .get("arguments")
                                .and_then(|v| v.as_str())
                                .unwrap_or("{}");

                            // Parse arguments JSON
                            let tool_input = if arguments_str.trim().is_empty() {
                                crate::core::tools::ToolInput::Structured(serde_json::json!({}))
                            } else {
                                match serde_json::from_str::<serde_json::Value>(arguments_str) {
                                    Ok(args) => {
                                        // Check for __arg1 special case
                                        if let Some(arg1) = args.get("__arg1") {
                                            if let Some(s) = arg1.as_str() {
                                                crate::core::tools::ToolInput::String(s.to_string())
                                            } else {
                                                crate::core::tools::ToolInput::Structured(args)
                                            }
                                        } else {
                                            crate::core::tools::ToolInput::Structured(args)
                                        }
                                    }
                                    Err(e) => {
                                        return Err(crate::core::error::Error::Agent(format!(
                                            "Failed to parse tool arguments: {e}"
                                        )));
                                    }
                                }
                            };

                            let log = if content.as_text().is_empty() {
                                format!("Calling tool: {function_name} with {tool_input:?}")
                            } else {
                                content.as_text()
                            };

                            return Ok(AgentDecision::Action(AgentAction {
                                tool: function_name,
                                tool_input,
                                log,
                            }));
                        }
                    }
                }
            }
        }

        // No tool calls - agent is done
        let content = generation.message.content();
        let content_text = content.as_text();
        Ok(AgentDecision::Finish(AgentFinish {
            output: content_text.clone(),
            log: content_text,
        }))
    }

    fn input_keys(&self) -> Vec<String> {
        vec!["input".to_string()]
    }

    fn output_keys(&self) -> Vec<String> {
        vec!["output".to_string()]
    }
}
