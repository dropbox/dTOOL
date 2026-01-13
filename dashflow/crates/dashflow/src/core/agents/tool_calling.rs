use crate::core::error::Result;

use super::{Agent, AgentAction, AgentDecision, AgentFinish, AgentStep};

/// Agent that uses a chat model's native tool calling functionality
///
/// This agent works with chat models that support tool calling (e.g., `OpenAI`, Anthropic).
/// It converts tools into a format the model understands, sends messages to the model,
/// and parses the model's `tool_calls` to decide actions.
///
/// # Architecture
///
/// 1. Agent receives input and intermediate steps
/// 2. Formats into messages (system prompt + history + current input)
/// 3. Calls chat model with tools parameter
/// 4. If model returns `tool_calls` -> `AgentAction`
/// 5. If model returns regular content -> `AgentFinish`
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::agents::ToolCallingAgent;
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::tools::Tool;
///
/// let agent = ToolCallingAgent::new(
///     chat_model,
///     vec![calculator_tool, search_tool],
///     "You are a helpful assistant that can use tools to answer questions."
/// );
/// ```
pub struct ToolCallingAgent {
    /// The chat model to use for reasoning
    chat_model: std::sync::Arc<dyn crate::core::language_models::ChatModel>,
    /// Tool definitions for passing to language model
    tool_definitions: Vec<crate::core::language_models::ToolDefinition>,
    /// System prompt that guides the agent's behavior
    system_prompt: String,
}

impl ToolCallingAgent {
    /// Create a new tool calling agent
    ///
    /// # Arguments
    ///
    /// * `chat_model` - Chat model that supports tool calling
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
            tool_definitions,
            system_prompt: system_prompt.into(),
        }
    }

    pub(super) fn steps_to_messages(
        &self,
        steps: &[AgentStep],
    ) -> Vec<crate::core::messages::BaseMessage> {
        use crate::core::messages::{BaseMessage, Message, MessageContent, ToolCall};

        let mut messages = Vec::new();

        for step in steps {
            let tool_call = ToolCall {
                id: format!("call_{}", uuid::Uuid::new_v4()),
                name: step.action.tool.clone(),
                args: match &step.action.tool_input {
                    crate::core::tools::ToolInput::String(s) => serde_json::json!({"input": s}),
                    crate::core::tools::ToolInput::Structured(v) => v.clone(),
                },
                tool_type: "tool_call".to_string(),
                index: None,
            };

            let tool_call_id = tool_call.id.clone();

            messages.push(BaseMessage::from(Message::AI {
                content: MessageContent::Text(step.action.log.clone()),
                tool_calls: vec![tool_call],
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: crate::core::messages::BaseMessageFields::default(),
            }));

            messages.push(BaseMessage::from(Message::tool(
                step.observation.clone(),
                tool_call_id,
            )));
        }

        messages
    }
}

#[async_trait::async_trait]
impl Agent for ToolCallingAgent {
    async fn plan(&self, input: &str, intermediate_steps: &[AgentStep]) -> Result<AgentDecision> {
        use crate::core::messages::{BaseMessage, Message};

        let mut messages = vec![BaseMessage::from(Message::system(
            self.system_prompt.clone(),
        ))];

        messages.extend(self.steps_to_messages(intermediate_steps));
        messages.push(BaseMessage::from(Message::human(input)));

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

        let generation = result.generations.first().ok_or_else(|| {
            crate::core::error::Error::Agent("No generations returned from chat model".to_string())
        })?;

        if let Message::AI {
            tool_calls,
            content,
            ..
        } = &generation.message
        {
            if !tool_calls.is_empty() {
                let tool_call = &tool_calls[0];

                let tool_input = if tool_call.args.is_object() || tool_call.args.is_array() {
                    crate::core::tools::ToolInput::Structured(tool_call.args.clone())
                } else {
                    crate::core::tools::ToolInput::String(tool_call.args.to_string())
                };

                return Ok(AgentDecision::Action(AgentAction {
                    tool: tool_call.name.clone(),
                    tool_input,
                    log: content.as_text(),
                }));
            }
        }

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
    use std::sync::Arc;

    // ===================== Mock Tool =====================

    struct MockTool {
        name: String,
        description: String,
    }

    impl MockTool {
        fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                description: description.into(),
            }
        }
    }

    #[async_trait::async_trait]
    impl crate::core::tools::Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            &self.description
        }
        async fn _call(
            &self,
            _input: crate::core::tools::ToolInput,
        ) -> crate::core::error::Result<String> {
            Ok("mock result".to_string())
        }
    }

    // ===================== Mock ChatModel =====================

    struct MockChatModel {
        response: String,
        tool_calls: Vec<crate::core::messages::ToolCall>,
    }

    impl MockChatModel {
        fn with_text_response(text: impl Into<String>) -> Self {
            Self {
                response: text.into(),
                tool_calls: vec![],
            }
        }

        fn with_tool_call(
            tool_name: impl Into<String>,
            args: serde_json::Value,
        ) -> Self {
            Self {
                response: String::new(),
                tool_calls: vec![crate::core::messages::ToolCall {
                    id: "call_123".to_string(),
                    name: tool_name.into(),
                    args,
                    tool_type: "tool_call".to_string(),
                    index: None,
                }],
            }
        }
    }

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
            use crate::core::messages::{Message, MessageContent};

            let message = if !self.tool_calls.is_empty() {
                Message::AI {
                    content: MessageContent::Text(self.response.clone()),
                    tool_calls: self.tool_calls.clone(),
                    invalid_tool_calls: vec![],
                    usage_metadata: None,
                    fields: crate::core::messages::BaseMessageFields::default(),
                }
            } else {
                Message::ai(self.response.clone())
            };

            Ok(crate::core::language_models::ChatResult {
                generations: vec![crate::core::language_models::ChatGeneration {
                    message: crate::core::messages::BaseMessage::from(message),
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

    // ===================== ToolCallingAgent Constructor =====================

    #[test]
    fn tool_calling_agent_stores_system_prompt() {
        let model = Arc::new(MockChatModel::with_text_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockTool::new("calculator", "Does math"));

        let agent = ToolCallingAgent::new(model, vec![tool], "You are helpful.");
        assert_eq!(agent.system_prompt, "You are helpful.");
    }

    #[test]
    fn tool_calling_agent_creates_tool_definitions() {
        let model = Arc::new(MockChatModel::with_text_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockTool::new("calculator", "Does math"));

        let agent = ToolCallingAgent::new(model, vec![tool], "prompt");
        assert_eq!(agent.tool_definitions.len(), 1);
        assert_eq!(agent.tool_definitions[0].name, "calculator");
    }

    #[test]
    fn tool_calling_agent_with_multiple_tools() {
        let model = Arc::new(MockChatModel::with_text_response("response"));
        let tool1: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockTool::new("calculator", "Does math"));
        let tool2: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockTool::new("search", "Searches"));

        let agent = ToolCallingAgent::new(model, vec![tool1, tool2], "prompt");
        assert_eq!(agent.tool_definitions.len(), 2);
    }

    #[test]
    fn tool_calling_agent_with_no_tools() {
        let model = Arc::new(MockChatModel::with_text_response("response"));
        let agent = ToolCallingAgent::new(model, vec![], "prompt");
        assert!(agent.tool_definitions.is_empty());
    }

    // ===================== input_keys / output_keys =====================

    #[test]
    fn tool_calling_agent_input_keys() {
        let model = Arc::new(MockChatModel::with_text_response("response"));
        let agent = ToolCallingAgent::new(model, vec![], "prompt");
        assert_eq!(agent.input_keys(), vec!["input"]);
    }

    #[test]
    fn tool_calling_agent_output_keys() {
        let model = Arc::new(MockChatModel::with_text_response("response"));
        let agent = ToolCallingAgent::new(model, vec![], "prompt");
        assert_eq!(agent.output_keys(), vec!["output"]);
    }

    // ===================== steps_to_messages =====================

    #[test]
    fn steps_to_messages_empty_steps() {
        let model = Arc::new(MockChatModel::with_text_response("response"));
        let agent = ToolCallingAgent::new(model, vec![], "prompt");

        let messages = agent.steps_to_messages(&[]);
        assert!(messages.is_empty());
    }

    #[test]
    fn steps_to_messages_single_step() {
        let model = Arc::new(MockChatModel::with_text_response("response"));
        let agent = ToolCallingAgent::new(model, vec![], "prompt");

        let step = AgentStep {
            action: AgentAction {
                tool: "calculator".to_string(),
                tool_input: ToolInput::String("2+2".to_string()),
                log: "Calculating".to_string(),
            },
            observation: "4".to_string(),
        };

        let messages = agent.steps_to_messages(&[step]);
        // Should produce 2 messages: AI with tool call, Tool response
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn steps_to_messages_multiple_steps() {
        let model = Arc::new(MockChatModel::with_text_response("response"));
        let agent = ToolCallingAgent::new(model, vec![], "prompt");

        let steps = vec![
            AgentStep {
                action: AgentAction {
                    tool: "tool1".to_string(),
                    tool_input: ToolInput::String("input1".to_string()),
                    log: "log1".to_string(),
                },
                observation: "result1".to_string(),
            },
            AgentStep {
                action: AgentAction {
                    tool: "tool2".to_string(),
                    tool_input: ToolInput::String("input2".to_string()),
                    log: "log2".to_string(),
                },
                observation: "result2".to_string(),
            },
        ];

        let messages = agent.steps_to_messages(&steps);
        assert_eq!(messages.len(), 4); // 2 per step
    }

    #[test]
    fn steps_to_messages_with_structured_input() {
        let model = Arc::new(MockChatModel::with_text_response("response"));
        let agent = ToolCallingAgent::new(model, vec![], "prompt");

        let step = AgentStep {
            action: AgentAction {
                tool: "search".to_string(),
                tool_input: ToolInput::Structured(serde_json::json!({"query": "hello"})),
                log: "Searching".to_string(),
            },
            observation: "Found it".to_string(),
        };

        let messages = agent.steps_to_messages(&[step]);
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn steps_to_messages_ai_message_has_tool_calls() {
        let model = Arc::new(MockChatModel::with_text_response("response"));
        let agent = ToolCallingAgent::new(model, vec![], "prompt");

        let step = AgentStep {
            action: AgentAction {
                tool: "calculator".to_string(),
                tool_input: ToolInput::String("2+2".to_string()),
                log: "Calculating".to_string(),
            },
            observation: "4".to_string(),
        };

        let messages = agent.steps_to_messages(&[step]);
        // First message should be AI type
        assert_eq!(messages[0].message_type(), "ai");
    }

    #[test]
    fn steps_to_messages_tool_message_follows() {
        let model = Arc::new(MockChatModel::with_text_response("response"));
        let agent = ToolCallingAgent::new(model, vec![], "prompt");

        let step = AgentStep {
            action: AgentAction {
                tool: "calculator".to_string(),
                tool_input: ToolInput::String("2+2".to_string()),
                log: "Calculating".to_string(),
            },
            observation: "4".to_string(),
        };

        let messages = agent.steps_to_messages(&[step]);
        // Second message should be tool type
        assert_eq!(messages[1].message_type(), "tool");
    }

    // ===================== plan =====================

    #[tokio::test]
    async fn plan_returns_finish_when_no_tool_calls() {
        let model = Arc::new(MockChatModel::with_text_response("The answer is 42"));
        let agent = ToolCallingAgent::new(model, vec![], "You are helpful.");

        let result = agent.plan("What is 6*7?", &[]).await.expect("plan");
        match result {
            AgentDecision::Finish(finish) => {
                assert_eq!(finish.output, "The answer is 42");
            }
            AgentDecision::Action(_) => panic!("Expected Finish, got Action"),
        }
    }

    #[tokio::test]
    async fn plan_returns_action_when_tool_call() {
        let model = Arc::new(MockChatModel::with_tool_call(
            "calculator",
            serde_json::json!({"expression": "2+2"}),
        ));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockTool::new("calculator", "Does math"));
        let agent = ToolCallingAgent::new(model, vec![tool], "You are helpful.");

        let result = agent.plan("What is 2+2?", &[]).await.expect("plan");
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "calculator");
            }
            AgentDecision::Finish(_) => panic!("Expected Action, got Finish"),
        }
    }

    #[tokio::test]
    async fn plan_includes_intermediate_steps() {
        let model = Arc::new(MockChatModel::with_text_response("Final answer"));
        let agent = ToolCallingAgent::new(model, vec![], "prompt");

        let steps = vec![AgentStep {
            action: AgentAction {
                tool: "search".to_string(),
                tool_input: ToolInput::String("query".to_string()),
                log: "Searching".to_string(),
            },
            observation: "Found results".to_string(),
        }];

        // Should not panic - steps are converted to messages
        let result = agent.plan("Continue?", &steps).await.expect("plan");
        assert!(matches!(result, AgentDecision::Finish(_)));
    }

    #[tokio::test]
    async fn plan_action_has_structured_input_for_object() {
        let model = Arc::new(MockChatModel::with_tool_call(
            "search",
            serde_json::json!({"query": "test"}),
        ));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockTool::new("search", "Searches"));
        let agent = ToolCallingAgent::new(model, vec![tool], "prompt");

        let result = agent.plan("Search for test", &[]).await.expect("plan");
        if let AgentDecision::Action(action) = result {
            match action.tool_input {
                ToolInput::Structured(v) => {
                    assert_eq!(v.get("query").unwrap().as_str(), Some("test"));
                }
                _ => panic!("Expected Structured input"),
            }
        }
    }

    #[tokio::test]
    async fn plan_action_has_string_input_for_primitive() {
        let model = Arc::new(MockChatModel::with_tool_call(
            "echo",
            serde_json::json!("hello"),
        ));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockTool::new("echo", "Echoes"));
        let agent = ToolCallingAgent::new(model, vec![tool], "prompt");

        let result = agent.plan("Echo hello", &[]).await.expect("plan");
        if let AgentDecision::Action(action) = result {
            match action.tool_input {
                ToolInput::String(s) => {
                    assert!(s.contains("hello"));
                }
                _ => panic!("Expected String input"),
            }
        }
    }
}
