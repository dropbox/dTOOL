//! JSON chat agent implementation.

use crate::core::error::Result;

use super::{Agent, AgentAction, AgentDecision, AgentFinish, AgentStep};

/// JSON Chat Agent that uses JSON formatting with chat-based scratchpad
///
/// This agent formats tool invocations as JSON in markdown code blocks,
/// similar to `StructuredChatAgent` but with a chat-based scratchpad format.
/// The scratchpad alternates between AI messages (actions) and Human messages
/// (observations), making it more suitable for chat models.
///
/// Corresponds to `create_json_chat_agent()` in Python `DashFlow`.
pub struct JsonChatAgent {
    /// The chat model to use for reasoning
    chat_model: std::sync::Arc<dyn crate::core::language_models::ChatModel>,

    /// Tool definitions for passing to language model
    tool_definitions: Vec<crate::core::language_models::ToolDefinition>,

    /// System prompt that guides the agent's behavior
    system_prompt: String,

    /// Template for formatting tool responses/observations
    /// Default: "{observation}"
    /// Can include context like "TOOL RESPONSE: {observation}"
    template_tool_response: String,

    /// Optional stop sequences to prevent hallucinations
    /// Python default: ["\\nObservation"] when enabled
    stop_sequences: Vec<String>,
}

impl JsonChatAgent {
    /// Create a new JSON Chat agent
    ///
    /// # Arguments
    ///
    /// * `chat_model` - Chat model for reasoning
    /// * `tools` - List of tools the agent can use
    /// * `system_prompt` - Instructions for the agent
    /// * `template_tool_response` - Template for formatting observations (default: "{observation}")
    /// * `stop_sequences` - Optional stop sequences to prevent hallucinations
    pub fn new(
        chat_model: std::sync::Arc<dyn crate::core::language_models::ChatModel>,
        tools: Vec<std::sync::Arc<dyn crate::core::tools::Tool>>,
        system_prompt: impl Into<String>,
        template_tool_response: impl Into<String>,
        stop_sequences: Vec<String>,
    ) -> Self {
        let tool_definitions = crate::core::tools::tools_to_definitions(&tools);
        Self {
            chat_model,
            tool_definitions,
            system_prompt: system_prompt.into(),
            template_tool_response: template_tool_response.into(),
            stop_sequences,
        }
    }

    /// Create default system prompt for JSON chat
    ///
    /// Includes tool descriptions and JSON format instructions.
    /// Based on Python baseline prompt template.
    pub fn default_prompt(tools: &[std::sync::Arc<dyn crate::core::tools::Tool>]) -> String {
        let tool_descriptions: Vec<String> = tools
            .iter()
            .map(|tool| format!("{}: {}", tool.name(), tool.description()))
            .collect();

        let tool_names: Vec<String> = tools.iter().map(|tool| tool.name().to_string()).collect();

        format!(
            r#"Assistant is a large language model trained by OpenAI.

Assistant is designed to be able to assist with a wide range of tasks, from answering simple questions to providing in-depth explanations and discussions on a wide range of topics. As a language model, Assistant is able to generate human-like text based on the input it receives, allowing it to engage in natural-sounding conversations and provide responses that are coherent and relevant to the topic at hand.

Assistant is constantly learning and improving, and its capabilities are constantly evolving. It is able to process and understand large amounts of text, and can use this knowledge to provide accurate and informative responses to a wide range of questions. Additionally, Assistant is able to generate its own text based on the input it receives, allowing it to engage in discussions and provide explanations and descriptions on a wide range of topics.

Overall, Assistant is a powerful system that can help with a wide range of tasks and provide valuable insights and information on a wide range of topics. Whether you need help with a specific question or just want to have a conversation about a particular topic, Assistant is here to assist.

TOOLS
------
Assistant can ask the user to use tools to look up information that may be helpful in answering the users original question. The tools the human can use are:

{}

RESPONSE FORMAT INSTRUCTIONS
----------------------------

When responding to me, please output a response in one of two formats:

**Option 1:**
Use this if you want the human to use a tool.
Markdown code snippet formatted in the following schema:

```json
{{{{
    "action": string, \\\\ The action to take. Must be one of {}
    "action_input": string \\\\ The input to the action
}}}}
```

**Option #2:**
Use this if you want to respond directly to the human. Markdown code snippet formatted in the following schema:

```json
{{{{
    "action": "Final Answer",
    "action_input": string \\\\ You should put what you want to return to use here
}}}}
```"#,
            tool_descriptions.join("\n"),
            tool_names.join(", ")
        )
    }

    /// Default template for tool response formatting
    ///
    /// Matches Python baseline `TEMPLATE_TOOL_RESPONSE` constant
    #[must_use]
    pub fn default_tool_response_template() -> String {
        r"TOOL RESPONSE:
---------------------
{observation}

USER'S INPUT
--------------------

Okay, so what is the response to my last comment? If using information obtained from the tools you must mention it explicitly without mentioning the tool names - I have forgotten all TOOL RESPONSES! Remember to respond with a markdown code snippet of a json blob with a single action, and NOTHING else - even if you just want to respond to the user. Do NOT respond with anything except a JSON snippet no matter what!".to_string()
    }

    /// Format scratchpad from intermediate steps as messages
    ///
    /// Converts agent steps into alternating AI/Human messages:
    /// - AI message: contains the action.log (the agent's reasoning/action)
    /// - Human message: contains the observation formatted with `template_tool_response`
    ///
    /// This matches Python's `format_log_to_messages()` function.
    pub(super) fn format_scratchpad_messages(
        &self,
        steps: &[AgentStep],
    ) -> Vec<crate::core::messages::BaseMessage> {
        use crate::core::messages::{BaseMessage, Message};

        let mut messages = Vec::new();

        for step in steps {
            // AI message with the action log
            messages.push(BaseMessage::from(Message::ai(step.action.log.clone())));

            // Human message with formatted observation
            let observation_text = self
                .template_tool_response
                .replace("{observation}", &step.observation);
            messages.push(BaseMessage::from(Message::human(observation_text)));
        }

        messages
    }

    /// Parse JSON output with partial JSON support
    ///
    /// Uses more robust parsing than `StructuredChatAgent`:
    /// 1. Tries to parse as-is
    /// 2. Looks for JSON in markdown code blocks
    /// 3. Attempts partial JSON completion (handles incomplete JSON)
    ///
    /// This matches Python's `parse_json_markdown()` with `parse_partial_json`.
    pub(super) fn parse_output(&self, text: &str) -> Result<AgentDecision> {
        use crate::core::error::Error;

        // Try to parse with partial JSON support
        let parsed = self.parse_json_markdown(text).map_err(|e| {
            Error::OutputParsing(format!("Could not parse LLM output: {e}. Text: {text}"))
        })?;

        // Handle case where LLM returns an array (sometimes GPT does this)
        let response = if parsed.is_array() {
            parsed
                .as_array()
                .and_then(|arr| arr.first())
                .ok_or_else(|| Error::OutputParsing("Empty array in response".to_string()))?
        } else {
            &parsed
        };

        // Extract action field
        let action = response
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::OutputParsing(format!("Missing 'action' field in JSON: {response}"))
            })?;

        // Check if this is Final Answer
        if action == "Final Answer" {
            let action_input = response.get("action_input");
            let output = if let Some(input) = action_input {
                if let Some(s) = input.as_str() {
                    s.to_string()
                } else {
                    input.to_string()
                }
            } else {
                String::new()
            };

            return Ok(AgentDecision::Finish(AgentFinish::new(
                output,
                text.to_string(),
            )));
        }

        // Regular tool action
        let action_input = response.get("action_input");
        let tool_input = if let Some(input) = action_input {
            if input.is_string() {
                crate::core::tools::ToolInput::String(input.as_str().unwrap_or("").to_string())
            } else if input.is_null() {
                // Handle null as empty object
                crate::core::tools::ToolInput::Structured(serde_json::json!({}))
            } else {
                crate::core::tools::ToolInput::Structured(input.clone())
            }
        } else {
            // Default to empty object if no action_input provided
            crate::core::tools::ToolInput::Structured(serde_json::json!({}))
        };

        Ok(AgentDecision::Action(AgentAction::new(
            action,
            tool_input,
            text.to_string(),
        )))
    }

    /// Parse JSON from markdown with partial JSON support
    ///
    /// This is a simplified version of Python's `parse_json_markdown`.
    /// Full partial JSON parsing (auto-closing braces, etc.) would require
    /// more complex logic. This implementation handles the common cases:
    /// 1. Direct JSON parsing
    /// 2. JSON in markdown code blocks (```json or ```)
    /// 3. Basic whitespace/backtick stripping
    fn parse_json_markdown(&self, text: &str) -> Result<serde_json::Value> {
        use crate::core::error::Error;

        // Try direct parse first
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
            return Ok(value);
        }

        // Look for JSON in markdown code blocks
        // Regex: ```(?:json)?(.*?)``` with DOTALL flag
        let re = regex::Regex::new(r"(?s)```(?:json\s*)?(.*?)```")
            .map_err(|e| Error::InvalidInput(format!("Regex compilation failed: {e}")))?;

        let json_str = if let Some(captures) = re.captures(text) {
            captures.get(1).unwrap().as_str()
        } else {
            // No code block found, use entire text
            text
        };

        // Strip whitespace and backticks
        let json_str = json_str.trim_matches(&[' ', '\n', '\r', '\t', '`'][..]);

        // Try parsing the extracted JSON
        let value = serde_json::from_str::<serde_json::Value>(json_str).map_err(|e| {
            Error::OutputParsing(format!("Failed to parse JSON: {e}. Extracted: {json_str}"))
        })?;

        Ok(value)
    }
}

#[async_trait::async_trait]
impl Agent for JsonChatAgent {
    async fn plan(&self, input: &str, intermediate_steps: &[AgentStep]) -> Result<AgentDecision> {
        use crate::core::messages::{BaseMessage, Message};

        // Build message list starting with system prompt
        let mut messages = vec![BaseMessage::from(Message::system(
            self.system_prompt.clone(),
        ))];

        // Add scratchpad messages (alternating AI/Human for each step)
        let scratchpad_messages = self.format_scratchpad_messages(intermediate_steps);
        messages.extend(scratchpad_messages);

        // Add user input as final human message
        messages.push(BaseMessage::from(Message::human(input.to_string())));

        // Generate response with stop sequences and tool definitions
        let stop = if self.stop_sequences.is_empty() {
            None
        } else {
            Some(self.stop_sequences.as_slice())
        };
        let result = self
            .chat_model
            .generate(
                &messages,
                stop,
                Some(&self.tool_definitions),
                Some(&crate::core::language_models::ToolChoice::Auto),
                None,
            )
            .await?;

        // Get the first generation
        let generation = result.generations.first().ok_or_else(|| {
            crate::core::error::Error::Agent("No generations returned from chat model".to_string())
        })?;

        // Extract text
        let text = generation.message.content().as_text();

        // Parse output
        self.parse_output(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tools::{Tool, ToolInput};
    use std::sync::Arc;

    /// Mock tool for testing
    struct MockTool {
        name: String,
        description: String,
    }

    impl MockTool {
        fn new(name: &str, description: &str) -> Self {
            Self {
                name: name.to_string(),
                description: description.to_string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn args_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                }
            })
        }

        async fn _call(&self, _input: ToolInput) -> crate::core::error::Result<String> {
            Ok("mock result".to_string())
        }
    }

    /// Mock chat model for creating agents
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

    fn create_test_agent() -> JsonChatAgent {
        let chat_model: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(MockTool::new("calculator", "Calculate math expressions")),
            Arc::new(MockTool::new("search", "Search the web")),
        ];
        JsonChatAgent::new(
            chat_model,
            tools,
            "You are a helpful assistant.",
            "{observation}",
            vec![],
        )
    }

    fn create_test_agent_with_template() -> JsonChatAgent {
        let chat_model: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MockTool::new("tool1", "Description 1"))];
        JsonChatAgent::new(
            chat_model,
            tools,
            "System prompt",
            "TOOL RESPONSE: {observation}",
            vec!["\nObservation".to_string()],
        )
    }

    fn create_tools() -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(MockTool::new("calculator", "Calculate math expressions")),
            Arc::new(MockTool::new("search", "Search the web")),
        ]
    }

    // ============================================================================
    // Constructor Tests
    // ============================================================================

    #[test]
    fn test_json_chat_agent_new() {
        let agent = create_test_agent();
        assert_eq!(agent.system_prompt, "You are a helpful assistant.");
        assert_eq!(agent.tool_definitions.len(), 2);
        assert_eq!(agent.template_tool_response, "{observation}");
        assert!(agent.stop_sequences.is_empty());
    }

    #[test]
    fn test_json_chat_agent_with_stop_sequences() {
        let agent = create_test_agent_with_template();
        assert!(!agent.stop_sequences.is_empty());
        assert_eq!(agent.stop_sequences[0], "\nObservation");
    }

    #[test]
    fn test_json_chat_agent_with_custom_template() {
        let agent = create_test_agent_with_template();
        assert_eq!(agent.template_tool_response, "TOOL RESPONSE: {observation}");
    }

    #[test]
    fn test_json_chat_agent_empty_tools() {
        let chat_model: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn Tool>> = vec![];
        let agent = JsonChatAgent::new(chat_model, tools, "Prompt", "{obs}", vec![]);
        assert!(agent.tool_definitions.is_empty());
    }

    // ============================================================================
    // default_prompt Tests
    // ============================================================================

    #[test]
    fn test_default_prompt_contains_assistant_intro() {
        let tools = create_tools();
        let prompt = JsonChatAgent::default_prompt(&tools);

        assert!(prompt.contains("Assistant is a large language model"));
        assert!(prompt.contains("TOOLS"));
    }

    #[test]
    fn test_default_prompt_contains_tool_info() {
        let tools = create_tools();
        let prompt = JsonChatAgent::default_prompt(&tools);

        assert!(prompt.contains("calculator"));
        assert!(prompt.contains("Calculate math expressions"));
        assert!(prompt.contains("search"));
        assert!(prompt.contains("Search the web"));
    }

    #[test]
    fn test_default_prompt_contains_format_instructions() {
        let tools = create_tools();
        let prompt = JsonChatAgent::default_prompt(&tools);

        assert!(prompt.contains("RESPONSE FORMAT INSTRUCTIONS"));
        assert!(prompt.contains("Option 1"));
        assert!(prompt.contains("Option #2"));
        assert!(prompt.contains("Final Answer"));
    }

    #[test]
    fn test_default_prompt_lists_tool_names() {
        let tools = create_tools();
        let prompt = JsonChatAgent::default_prompt(&tools);

        // Should list available tools
        assert!(prompt.contains("calculator, search"));
    }

    #[test]
    fn test_default_prompt_empty_tools() {
        let tools: Vec<Arc<dyn Tool>> = vec![];
        let prompt = JsonChatAgent::default_prompt(&tools);

        // Should still have valid structure
        assert!(prompt.contains("Assistant"));
        assert!(prompt.contains("TOOLS"));
        assert!(prompt.contains("Final Answer"));
    }

    // ============================================================================
    // default_tool_response_template Tests
    // ============================================================================

    #[test]
    fn test_default_tool_response_template() {
        let template = JsonChatAgent::default_tool_response_template();

        assert!(template.contains("TOOL RESPONSE:"));
        assert!(template.contains("{observation}"));
        assert!(template.contains("USER'S INPUT"));
    }

    #[test]
    fn test_default_tool_response_template_contains_instructions() {
        let template = JsonChatAgent::default_tool_response_template();

        assert!(template.contains("markdown code snippet"));
        assert!(template.contains("json blob"));
    }

    // ============================================================================
    // format_scratchpad_messages Tests
    // ============================================================================

    #[test]
    fn test_format_scratchpad_messages_empty() {
        let agent = create_test_agent();
        let messages = agent.format_scratchpad_messages(&[]);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_format_scratchpad_messages_single_step() {
        let agent = create_test_agent();
        let step = AgentStep {
            action: AgentAction::new("calculator", ToolInput::from("2+2"), "Calculating..."),
            observation: "4".to_string(),
        };

        let messages = agent.format_scratchpad_messages(&[step]);

        assert_eq!(messages.len(), 2);
        // First message should be AI with the action log
        assert_eq!(messages[0].message_type(), "ai");
        assert!(messages[0].content().as_text().contains("Calculating..."));
        // Second message should be Human with observation
        assert_eq!(messages[1].message_type(), "human");
        assert!(messages[1].content().as_text().contains("4"));
    }

    #[test]
    fn test_format_scratchpad_messages_multiple_steps() {
        let agent = create_test_agent();
        let steps = vec![
            AgentStep {
                action: AgentAction::new("calc", ToolInput::from("5*5"), "First step"),
                observation: "25".to_string(),
            },
            AgentStep {
                action: AgentAction::new("search", ToolInput::from("query"), "Second step"),
                observation: "Found results".to_string(),
            },
        ];

        let messages = agent.format_scratchpad_messages(&steps);

        assert_eq!(messages.len(), 4);
        // Alternating AI/Human pattern
        assert_eq!(messages[0].message_type(), "ai");
        assert_eq!(messages[1].message_type(), "human");
        assert_eq!(messages[2].message_type(), "ai");
        assert_eq!(messages[3].message_type(), "human");
    }

    #[test]
    fn test_format_scratchpad_messages_with_custom_template() {
        let agent = create_test_agent_with_template();
        let step = AgentStep {
            action: AgentAction::new("tool", ToolInput::from("input"), "Action log"),
            observation: "Tool result".to_string(),
        };

        let messages = agent.format_scratchpad_messages(&[step]);

        // Human message should use custom template
        assert!(messages[1]
            .content()
            .as_text()
            .contains("TOOL RESPONSE: Tool result"));
    }

    #[test]
    fn test_format_scratchpad_messages_preserves_order() {
        let agent = create_test_agent();
        let steps = vec![
            AgentStep {
                action: AgentAction::new("t", ToolInput::from("a"), "Step 1"),
                observation: "Result 1".to_string(),
            },
            AgentStep {
                action: AgentAction::new("t", ToolInput::from("b"), "Step 2"),
                observation: "Result 2".to_string(),
            },
            AgentStep {
                action: AgentAction::new("t", ToolInput::from("c"), "Step 3"),
                observation: "Result 3".to_string(),
            },
        ];

        let messages = agent.format_scratchpad_messages(&steps);

        assert_eq!(messages.len(), 6);
        assert!(messages[0].content().as_text().contains("Step 1"));
        assert!(messages[2].content().as_text().contains("Step 2"));
        assert!(messages[4].content().as_text().contains("Step 3"));
    }

    // ============================================================================
    // parse_output Tests - Final Answer
    // ============================================================================

    #[test]
    fn test_parse_output_final_answer_string() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action": "Final Answer",
  "action_input": "The answer is 42"
}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Finish(finish) => {
                assert_eq!(finish.output, "The answer is 42");
            }
            _ => panic!("Expected AgentFinish"),
        }
    }

    #[test]
    fn test_parse_output_final_answer_json() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action": "Final Answer",
  "action_input": {"result": 42}
}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Finish(finish) => {
                assert!(finish.output.contains("42"));
            }
            _ => panic!("Expected AgentFinish"),
        }
    }

    #[test]
    fn test_parse_output_final_answer_empty_input() {
        let agent = create_test_agent();
        let output = r#"```json
{"action": "Final Answer"}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Finish(finish) => {
                assert_eq!(finish.output, "");
            }
            _ => panic!("Expected AgentFinish"),
        }
    }

    // ============================================================================
    // parse_output Tests - Tool Actions
    // ============================================================================

    #[test]
    fn test_parse_output_tool_action_string_input() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action": "calculator",
  "action_input": "10 + 20"
}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "calculator");
                match action.tool_input {
                    ToolInput::String(s) => assert_eq!(s, "10 + 20"),
                    _ => panic!("Expected string input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_tool_action_structured_input() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action": "search",
  "action_input": {"query": "rust", "max_results": 5}
}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "search");
                match action.tool_input {
                    ToolInput::Structured(v) => {
                        assert_eq!(v["query"], "rust");
                        assert_eq!(v["max_results"], 5);
                    }
                    _ => panic!("Expected structured input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_tool_action_null_input() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action": "tool",
  "action_input": null
}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                // Null input should become empty object
                match action.tool_input {
                    ToolInput::Structured(v) => assert!(v.as_object().unwrap().is_empty()),
                    _ => panic!("Expected structured input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_tool_action_no_input() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action": "tool"
}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                // Missing input should become empty object
                match action.tool_input {
                    ToolInput::Structured(v) => assert!(v.as_object().unwrap().is_empty()),
                    _ => panic!("Expected structured input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    // ============================================================================
    // parse_output Tests - JSON Parsing
    // ============================================================================

    #[test]
    fn test_parse_output_direct_json() {
        let agent = create_test_agent();
        // JSON without markdown code block
        let output = r#"{"action": "calculator", "action_input": "1+1"}"#;

        let result = agent.parse_output(output).unwrap();
        assert!(result.is_action());
    }

    #[test]
    fn test_parse_output_json_without_language() {
        let agent = create_test_agent();
        let output = r#"```
{"action": "calculator", "action_input": "2+2"}
```"#;

        let result = agent.parse_output(output).unwrap();
        assert!(result.is_action());
    }

    #[test]
    fn test_parse_output_json_with_surrounding_text() {
        let agent = create_test_agent();
        let output = r#"Let me calculate this for you.

```json
{"action": "calculator", "action_input": "5*5"}
```

I hope that helps!"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "calculator");
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_json_array() {
        let agent = create_test_agent();
        let output = r#"```json
[{"action": "search", "action_input": "query"}]
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "search");
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    // ============================================================================
    // parse_output Tests - Error Cases
    // ============================================================================

    #[test]
    fn test_parse_output_invalid_json() {
        let agent = create_test_agent();
        let output = r#"```json
{invalid json}
```"#;

        let result = agent.parse_output(output);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_output_missing_action() {
        let agent = create_test_agent();
        let output = r#"```json
{"action_input": "some input"}
```"#;

        let result = agent.parse_output(output);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("action"));
    }

    #[test]
    fn test_parse_output_empty_array() {
        let agent = create_test_agent();
        let output = r#"```json
[]
```"#;

        let result = agent.parse_output(output);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_output_non_string_action() {
        let agent = create_test_agent();
        let output = r#"```json
{"action": 123, "action_input": "test"}
```"#;

        let result = agent.parse_output(output);
        assert!(result.is_err());
    }

    // ============================================================================
    // parse_output Tests - Edge Cases
    // ============================================================================

    #[test]
    fn test_parse_output_preserves_log() {
        let agent = create_test_agent();
        let output = r#"Thinking about this...

```json
{"action": "calculator", "action_input": "100"}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.log, output);
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_unicode() {
        let agent = create_test_agent();
        let output = r#"```json
{"action": "search", "action_input": "Rust プログラミング"}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                match action.tool_input {
                    ToolInput::String(s) => assert!(s.contains("プログラミング")),
                    _ => panic!("Expected string input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_whitespace() {
        let agent = create_test_agent();
        let output = r#"```json

  {
    "action"  :  "calculator"  ,
    "action_input"  :  "1"
  }

```"#;

        let result = agent.parse_output(output).unwrap();
        assert!(result.is_action());
    }

    #[test]
    fn test_parse_output_nested_json() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action": "api",
  "action_input": {
    "url": "/api/users",
    "body": {"name": "Test", "items": [1, 2, 3]}
  }
}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                match action.tool_input {
                    ToolInput::Structured(v) => {
                        assert_eq!(v["url"], "/api/users");
                        assert_eq!(v["body"]["name"], "Test");
                        assert_eq!(v["body"]["items"][0], 1);
                    }
                    _ => panic!("Expected structured input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    // ============================================================================
    // Integration Tests
    // ============================================================================

    #[test]
    fn test_agent_workflow() {
        let agent = create_test_agent();

        // Step 1: Empty scratchpad
        let messages = agent.format_scratchpad_messages(&[]);
        assert!(messages.is_empty());

        // Step 2: Parse tool action
        let llm_output = r#"```json
{"action": "calculator", "action_input": "5 + 5"}
```"#;
        let decision = agent.parse_output(llm_output).unwrap();
        let action = decision.as_action().unwrap();
        assert_eq!(action.tool, "calculator");

        // Step 3: Create step and check scratchpad
        let step = AgentStep {
            action: action.clone(),
            observation: "10".to_string(),
        };
        let messages = agent.format_scratchpad_messages(&[step]);
        assert_eq!(messages.len(), 2);
        assert!(messages[1].content().as_text().contains("10"));

        // Step 4: Parse final answer
        let final_output = r#"```json
{"action": "Final Answer", "action_input": "The result is 10"}
```"#;
        let final_decision = agent.parse_output(final_output).unwrap();
        assert!(final_decision.is_finish());
        let finish = final_decision.as_finish().unwrap();
        assert_eq!(finish.output, "The result is 10");
    }

    #[test]
    fn test_agent_multi_step_workflow() {
        let agent = create_test_agent();

        // Create multiple steps
        let steps = vec![
            AgentStep {
                action: AgentAction::new("calc", ToolInput::from("a"), "Step A"),
                observation: "Result A".to_string(),
            },
            AgentStep {
                action: AgentAction::new("calc", ToolInput::from("b"), "Step B"),
                observation: "Result B".to_string(),
            },
        ];

        let messages = agent.format_scratchpad_messages(&steps);

        // Should have 4 messages: AI, Human, AI, Human
        assert_eq!(messages.len(), 4);

        // Verify content
        assert!(messages[0].content().as_text().contains("Step A"));
        assert!(messages[1].content().as_text().contains("Result A"));
        assert!(messages[2].content().as_text().contains("Step B"));
        assert!(messages[3].content().as_text().contains("Result B"));
    }
}
