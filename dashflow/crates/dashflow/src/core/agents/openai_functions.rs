use crate::core::error::Result;

use super::{Agent, AgentAction, AgentDecision, AgentFinish, AgentStep};

/// `OpenAI` Functions Agent that uses `OpenAI`'s function calling API
///
/// This agent is specifically designed for `OpenAI`'s older function calling API
/// (as opposed to the newer "tools" API). It formats tools as `OpenAI` function
/// definitions and parses the model's `function_call` responses.
///
/// # Pattern
///
/// 1. Agent receives input and intermediate steps
/// 2. Formats tools as `OpenAI` function definitions
/// 3. LLM responds with either:
///    - `function_call` in `additional_kwargs` → `AgentAction`
///    - Regular message content → `AgentFinish`
/// 4. Function results are formatted as `FunctionMessage` for scratchpad
///
/// # Comparison to Other Agents
///
/// **`OpenAIFunctionsAgent` (this)**:
/// - Uses `OpenAI`'s older `function_call` format
/// - Compatible with models that support `functions` parameter
/// - Uses `Message::Function` for scratchpad
///
/// **`ToolCallingAgent`**:
/// - Uses modern tool calling API (`OpenAI`'s `tools` parameter)
/// - Uses `Message::Tool` for scratchpad
/// - More flexible, works with multiple providers
///
/// **`ReActAgent`**:
/// - Works with any LLM (no function calling required)
/// - Uses prompt-based reasoning and text parsing
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::agents::OpenAIFunctionsAgent;
/// use dashflow::core::language_models::ChatModel;
/// use std::sync::Arc;
///
/// async fn example(llm: Arc<dyn ChatModel>) {
///     let tools = vec![/* tools */];
///
///     let agent = OpenAIFunctionsAgent::new(
///         llm,
///         tools,
///         "You are a helpful assistant with access to functions."
///     );
///
///     // Use with AgentExecutor
/// }
/// ```
pub struct OpenAIFunctionsAgent {
    /// The chat model to use for reasoning (should support `OpenAI` functions)
    chat_model: std::sync::Arc<dyn crate::core::language_models::ChatModel>,

    /// Tools available to the agent (used in test methods)
    #[allow(dead_code)] // Test: Accessed by tools_to_functions() in test code
    tools: Vec<std::sync::Arc<dyn crate::core::tools::Tool>>,

    /// Tool definitions for passing to language model
    tool_definitions: Vec<crate::core::language_models::ToolDefinition>,

    /// System prompt that guides the agent's behavior
    system_prompt: String,
}

impl OpenAIFunctionsAgent {
    /// Create a new `OpenAI` Functions agent
    ///
    /// # Arguments
    ///
    /// * `chat_model` - Chat model that supports `OpenAI` function calling
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

    /// Convert tools to `OpenAI` function definitions
    ///
    /// Returns a JSON array of function definitions in `OpenAI` format:
    /// ```json
    /// {
    ///   "name": "function_name",
    ///   "description": "What the function does",
    ///   "parameters": { /* JSON Schema */ }
    /// }
    /// ```
    #[cfg(test)]
    pub(super) fn tools_to_functions(&self) -> Vec<serde_json::Value> {
        self.tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "parameters": tool.args_schema()
                })
            })
            .collect()
    }

    /// Convert intermediate steps into `FunctionMessage` format
    ///
    /// Each step becomes:
    /// 1. `AIMessage` with `function_call` in `additional_kwargs`
    /// 2. `FunctionMessage` with the observation result
    pub(super) fn steps_to_messages(
        &self,
        steps: &[AgentStep],
    ) -> Vec<crate::core::messages::BaseMessage> {
        use crate::core::messages::{BaseMessage, BaseMessageFields, Message, MessageContent};

        let mut messages = Vec::new();

        for step in steps {
            // Add AIMessage with function_call in additional_kwargs
            let function_call_json = serde_json::json!({
                "name": step.action.tool,
                "arguments": match &step.action.tool_input {
                    crate::core::tools::ToolInput::String(s) => {
                        // Simple string input - wrap in a {"__arg1": value} structure
                        serde_json::json!({"__arg1": s}).to_string()
                    }
                    crate::core::tools::ToolInput::Structured(v) => {
                        // Already structured - serialize as-is
                        v.to_string()
                    }
                }
            });

            let mut ai_fields = BaseMessageFields::default();
            ai_fields
                .additional_kwargs
                .insert("function_call".to_string(), function_call_json);

            messages.push(BaseMessage::from(Message::AI {
                content: MessageContent::Text(step.action.log.clone()),
                tool_calls: vec![],
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: ai_fields,
            }));

            // Add FunctionMessage with observation
            messages.push(BaseMessage::from(Message::Function {
                content: MessageContent::Text(step.observation.clone()),
                name: step.action.tool.clone(),
                fields: BaseMessageFields::default(),
            }));
        }

        messages
    }
}

#[async_trait::async_trait]
impl Agent for OpenAIFunctionsAgent {
    async fn plan(&self, input: &str, intermediate_steps: &[AgentStep]) -> Result<AgentDecision> {
        use crate::core::messages::{BaseMessage, Message};

        // Build message list
        let mut messages = vec![BaseMessage::from(Message::system(
            self.system_prompt.clone(),
        ))];

        // Add intermediate steps as function messages
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

        // Check if the message has a function_call in additional_kwargs
        if let Message::AI {
            fields, content, ..
        } = &generation.message
        {
            if let Some(function_call) = fields.additional_kwargs.get("function_call") {
                // Model wants to call a function
                let function_name = function_call
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::core::error::Error::Agent(
                            "function_call missing 'name' field".to_string(),
                        )
                    })?
                    .to_string();

                let arguments_str = function_call
                    .get("arguments")
                    .and_then(|v| v.as_str())
                    .unwrap_or("{}");

                // Parse arguments JSON
                let tool_input = if arguments_str.trim().is_empty() {
                    // Empty string means no arguments
                    crate::core::tools::ToolInput::Structured(serde_json::json!({}))
                } else {
                    match serde_json::from_str::<serde_json::Value>(arguments_str) {
                        Ok(args) => {
                            // Check for __arg1 special case (old-style single string argument)
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
                                "Failed to parse function arguments: {e}"
                            )));
                        }
                    }
                };

                let log = if content.as_text().is_empty() {
                    format!("Calling function: {function_name} with {tool_input:?}")
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

        // No function call - agent is done
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tools::ToolInput;

    // ==========================================================================
    // Agent construction tests
    // ==========================================================================

    #[test]
    fn test_new_agent_with_empty_tools() {
        let agent = create_test_agent();
        assert_eq!(agent.input_keys(), vec!["input"]);
        assert_eq!(agent.output_keys(), vec!["output"]);
    }

    #[test]
    fn test_new_agent_with_system_prompt() {
        let agent = create_test_agent();
        assert_eq!(agent.system_prompt, "You are a helpful assistant.");
    }

    #[test]
    fn test_input_keys() {
        let agent = create_test_agent();
        assert_eq!(agent.input_keys(), vec!["input"]);
    }

    #[test]
    fn test_output_keys() {
        let agent = create_test_agent();
        assert_eq!(agent.output_keys(), vec!["output"]);
    }

    // ==========================================================================
    // tools_to_functions tests
    // ==========================================================================

    #[test]
    fn test_tools_to_functions_empty() {
        let agent = create_test_agent();
        let functions = agent.tools_to_functions();
        assert!(functions.is_empty());
    }

    // ==========================================================================
    // steps_to_messages tests
    // ==========================================================================

    #[test]
    fn test_steps_to_messages_empty() {
        let agent = create_test_agent();
        let steps: Vec<AgentStep> = vec![];
        let messages = agent.steps_to_messages(&steps);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_steps_to_messages_single_step_string_input() {
        let agent = create_test_agent();
        let steps = vec![AgentStep {
            action: AgentAction {
                tool: "calculator".to_string(),
                tool_input: ToolInput::String("2+2".to_string()),
                log: "Calculating sum".to_string(),
            },
            observation: "4".to_string(),
        }];

        let messages = agent.steps_to_messages(&steps);

        // Should produce 2 messages: AI message with function_call + Function message with result
        assert_eq!(messages.len(), 2);

        // First message should be AI message with function_call
        let ai_message = &messages[0];
        let additional_kwargs = get_additional_kwargs(ai_message);
        assert!(additional_kwargs.contains_key("function_call"));

        let function_call = additional_kwargs.get("function_call").unwrap();
        assert_eq!(function_call["name"], "calculator");
        // Arguments should contain the string input wrapped in __arg1
        let args_str = function_call["arguments"].as_str().unwrap();
        assert!(args_str.contains("__arg1"));
        assert!(args_str.contains("2+2"));

        // Second message should be Function message with observation
        let func_message = &messages[1];
        assert_eq!(func_message.content().as_text(), "4");
    }

    #[test]
    fn test_steps_to_messages_single_step_structured_input() {
        let agent = create_test_agent();
        let steps = vec![AgentStep {
            action: AgentAction {
                tool: "search".to_string(),
                tool_input: ToolInput::Structured(serde_json::json!({
                    "query": "rust programming",
                    "limit": 10
                })),
                log: "Searching".to_string(),
            },
            observation: "Found 5 results".to_string(),
        }];

        let messages = agent.steps_to_messages(&steps);

        assert_eq!(messages.len(), 2);

        let ai_message = &messages[0];
        let additional_kwargs = get_additional_kwargs(ai_message);
        let function_call = additional_kwargs.get("function_call").unwrap();
        assert_eq!(function_call["name"], "search");

        // Arguments should be the serialized JSON
        let args_str = function_call["arguments"].as_str().unwrap();
        assert!(args_str.contains("query"));
        assert!(args_str.contains("rust programming"));
    }

    #[test]
    fn test_steps_to_messages_multiple_steps() {
        let agent = create_test_agent();
        let steps = vec![
            AgentStep {
                action: AgentAction {
                    tool: "search".to_string(),
                    tool_input: ToolInput::String("weather".to_string()),
                    log: "Searching for weather".to_string(),
                },
                observation: "Sunny, 72°F".to_string(),
            },
            AgentStep {
                action: AgentAction {
                    tool: "calculator".to_string(),
                    tool_input: ToolInput::String("72*2".to_string()),
                    log: "Calculating".to_string(),
                },
                observation: "144".to_string(),
            },
        ];

        let messages = agent.steps_to_messages(&steps);

        // 2 messages per step = 4 total
        assert_eq!(messages.len(), 4);

        // First step: AI + Function
        let ai1 = &messages[0];
        assert!(get_additional_kwargs(ai1).contains_key("function_call"));

        let func1 = &messages[1];
        assert_eq!(func1.content().as_text(), "Sunny, 72°F");

        // Second step: AI + Function
        let ai2 = &messages[2];
        let function_call = get_additional_kwargs(ai2).get("function_call").unwrap().clone();
        assert_eq!(function_call["name"], "calculator");

        let func2 = &messages[3];
        assert_eq!(func2.content().as_text(), "144");
    }

    #[test]
    fn test_steps_to_messages_preserves_log() {
        let agent = create_test_agent();
        let steps = vec![AgentStep {
            action: AgentAction {
                tool: "test".to_string(),
                tool_input: ToolInput::String("input".to_string()),
                log: "This is the reasoning log".to_string(),
            },
            observation: "result".to_string(),
        }];

        let messages = agent.steps_to_messages(&steps);
        let ai_message = &messages[0];

        // The AI message content should contain the log
        assert_eq!(ai_message.content().as_text(), "This is the reasoning log");
    }

    #[test]
    fn test_steps_to_messages_empty_tool_input() {
        let agent = create_test_agent();
        let steps = vec![AgentStep {
            action: AgentAction {
                tool: "get_time".to_string(),
                tool_input: ToolInput::String(String::new()),
                log: "Getting time".to_string(),
            },
            observation: "12:00 PM".to_string(),
        }];

        let messages = agent.steps_to_messages(&steps);
        assert_eq!(messages.len(), 2);

        let additional_kwargs = get_additional_kwargs(&messages[0]);
        let function_call = additional_kwargs.get("function_call").unwrap();
        // Empty string still gets wrapped in __arg1
        let args_str = function_call["arguments"].as_str().unwrap();
        assert!(args_str.contains("__arg1"));
    }

    /// Helper to extract additional_kwargs from a Message
    fn get_additional_kwargs(
        msg: &crate::core::messages::Message,
    ) -> &std::collections::HashMap<String, serde_json::Value> {
        use crate::core::messages::Message;
        match msg {
            Message::AI { fields, .. } => &fields.additional_kwargs,
            Message::Human { fields, .. } => &fields.additional_kwargs,
            Message::System { fields, .. } => &fields.additional_kwargs,
            Message::Tool { fields, .. } => &fields.additional_kwargs,
            Message::Function { fields, .. } => &fields.additional_kwargs,
        }
    }

    // ==========================================================================
    // Test helpers
    // ==========================================================================

    /// Mock chat model for creating agents (not used for actual LLM calls in these tests)
    struct MockChatModel;

    #[async_trait::async_trait]
    impl crate::core::language_models::ChatModel for MockChatModel {
        async fn _generate(
            &self,
            _messages: &[crate::core::messages::BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[crate::core::language_models::ToolDefinition]>,
            _tool_choice: Option<&crate::core::language_models::ToolChoice>,
            _run_manager: Option<&crate::core::callbacks::CallbackManager>,
        ) -> crate::core::error::Result<crate::core::language_models::ChatResult> {
            Ok(crate::core::language_models::ChatResult {
                generations: vec![crate::core::language_models::ChatGeneration {
                    message: crate::core::messages::BaseMessage::from(
                        crate::core::messages::Message::ai("mock response"),
                    ),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    fn create_test_agent() -> OpenAIFunctionsAgent {
        use std::sync::Arc;

        let mock_model: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn crate::core::tools::Tool>> = vec![];
        OpenAIFunctionsAgent::new(mock_model, tools, "You are a helpful assistant.")
    }
}
