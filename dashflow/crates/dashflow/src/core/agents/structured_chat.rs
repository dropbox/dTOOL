use std::sync::OnceLock;

use crate::core::error::Result;

use super::{Agent, AgentAction, AgentDecision, AgentFinish, AgentStep};

/// Structured Chat Agent that uses JSON-formatted actions
///
/// This agent expects LLM output in the format:
/// ```json
/// {
///   "action": "tool_name",
///   "action_input": {...}
/// }
/// ```
///
/// Or for final answer:
/// ```json
/// {
///   "action": "Final Answer",
///   "action_input": "the answer"
/// }
/// ```
///
/// The JSON should be wrapped in markdown code blocks (with or without language specifier).
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::agents::StructuredChatAgent;
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::tools::Tool;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let chat_model: Arc<dyn ChatModel> = todo!();
/// # let tools: Vec<Arc<dyn Tool>> = vec![];
/// let agent = StructuredChatAgent::new(
///     chat_model,
///     tools,
///     "You are a helpful assistant that uses tools to solve problems."
/// );
/// # Ok(())
/// # }
/// ```
pub struct StructuredChatAgent {
    /// The chat model to use for reasoning
    chat_model: std::sync::Arc<dyn crate::core::language_models::ChatModel>,
    /// Tool definitions for passing to language model
    tool_definitions: Vec<crate::core::language_models::ToolDefinition>,
    /// System prompt that guides the agent's behavior
    system_prompt: String,
}

impl StructuredChatAgent {
    /// Create a new Structured Chat agent
    ///
    /// # Arguments
    ///
    /// * `chat_model` - Chat model for reasoning
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

    /// Create default system prompt for structured chat
    ///
    /// Includes tool descriptions and instructions for JSON formatting
    pub fn default_prompt(tools: &[std::sync::Arc<dyn crate::core::tools::Tool>]) -> String {
        let tool_descriptions: Vec<String> = tools
            .iter()
            .map(|tool| {
                format!(
                    "{}: {}, args: {}",
                    tool.name(),
                    tool.description(),
                    serde_json::to_string(&tool.args_schema()).unwrap_or_else(|_| "{}".to_string())
                )
            })
            .collect();

        let tool_names: Vec<String> = tools.iter().map(|tool| tool.name().to_string()).collect();

        format!(
            r#"Respond to the human as helpfully and accurately as possible. You have access to the following tools:

{}

Use a json blob to specify a tool by providing an action key (tool name) and an action_input key (tool input).

Valid "action" values: "Final Answer" or {}

Provide only ONE action per $JSON_BLOB, as shown:

```
{{
  "action": $TOOL_NAME,
  "action_input": $INPUT
}}
```

Follow this format:

Question: input question to answer
Thought: consider previous and subsequent steps
Action:
```
$JSON_BLOB
```
Observation: action result
... (repeat Thought/Action/Observation N times)
Thought: I know what to respond
Action:
```
{{
  "action": "Final Answer",
  "action_input": "Final response to human"
}}
```

Begin! Reminder to ALWAYS respond with a valid json blob of a single action. Use tools if necessary. Respond directly if appropriate. Format is Action:```$JSON_BLOB```then Observation:.
Thought:"#,
            tool_descriptions.join("\n"),
            tool_names.join(", ")
        )
    }

    pub(super) fn format_scratchpad(&self, steps: &[AgentStep]) -> String {
        if steps.is_empty() {
            return String::new();
        }

        let mut scratchpad = String::new();
        for step in steps {
            scratchpad.push_str(&format!("Thought: {}\n", step.action.log));
            scratchpad.push_str(&format!("Observation: {}\n", step.observation));
        }

        format!(
            "This was your previous work (but I haven't seen any of it! I only see what you return as final answer):\n{scratchpad}"
        )
    }

    pub(super) fn parse_output(&self, text: &str) -> Result<AgentDecision> {
        use crate::core::error::Error;

        static JSON_BLOCK_REGEX: OnceLock<regex::Regex> = OnceLock::new();
        let re = JSON_BLOCK_REGEX.get_or_init(|| {
            regex::Regex::new(r"(?s)```(?:json\s*)?(.+?)```").expect("valid regex")
        });

        let action_match = re.captures(text);

        if let Some(captures) = action_match {
            let json_str = captures.get(1).unwrap().as_str().trim();

            let parsed: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
                Error::OutputParsing(format!("Failed to parse JSON: {e}. Text: {json_str}"))
            })?;

            let response = if parsed.is_array() {
                parsed
                    .as_array()
                    .and_then(|arr| arr.first())
                    .ok_or_else(|| Error::OutputParsing("Empty array in response".to_string()))?
            } else {
                &parsed
            };

            let action = response
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::OutputParsing(format!("Missing 'action' field in JSON: {response}"))
                })?;

            let action_input = response.get("action_input");

            if action == "Final Answer" {
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

            let tool_input = if let Some(input) = action_input {
                if input.is_string() {
                    crate::core::tools::ToolInput::String(input.as_str().unwrap_or("").to_string())
                } else {
                    crate::core::tools::ToolInput::Structured(input.clone())
                }
            } else {
                crate::core::tools::ToolInput::Structured(serde_json::json!({}))
            };

            return Ok(AgentDecision::Action(AgentAction::new(
                action,
                tool_input,
                text.to_string(),
            )));
        }

        Ok(AgentDecision::Finish(AgentFinish::new(
            text.to_string(),
            text.to_string(),
        )))
    }
}

#[async_trait::async_trait]
impl Agent for StructuredChatAgent {
    async fn plan(&self, input: &str, intermediate_steps: &[AgentStep]) -> Result<AgentDecision> {
        use crate::core::messages::{BaseMessage, Message};

        let mut messages = vec![BaseMessage::from(Message::system(
            self.system_prompt.clone(),
        ))];

        let scratchpad = self.format_scratchpad(intermediate_steps);
        let user_message = if scratchpad.is_empty() {
            input.to_string()
        } else {
            format!("{input}\n\n{scratchpad}")
        };

        messages.push(BaseMessage::from(Message::human(user_message)));

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

        let text = generation.message.content().as_text();
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
                    "query": {"type": "string"}
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

    fn create_test_agent() -> StructuredChatAgent {
        let chat_model: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(MockTool::new("calculator", "Calculate math expressions")),
            Arc::new(MockTool::new("search", "Search the web")),
        ];
        StructuredChatAgent::new(chat_model, tools, "You are a helpful assistant.")
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
    fn test_structured_chat_agent_new() {
        let agent = create_test_agent();
        assert_eq!(agent.system_prompt, "You are a helpful assistant.");
        assert_eq!(agent.tool_definitions.len(), 2);
    }

    #[test]
    fn test_structured_chat_agent_new_with_string_prompt() {
        let chat_model: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn Tool>> = vec![];
        let agent = StructuredChatAgent::new(chat_model, tools, String::from("Custom system prompt"));
        assert_eq!(agent.system_prompt, "Custom system prompt");
    }

    #[test]
    fn test_structured_chat_agent_empty_tools() {
        let chat_model: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn Tool>> = vec![];
        let agent = StructuredChatAgent::new(chat_model, tools, "Prompt");
        assert!(agent.tool_definitions.is_empty());
    }

    // ============================================================================
    // default_prompt Tests
    // ============================================================================

    #[test]
    fn test_default_prompt_contains_tool_info() {
        let tools = create_tools();
        let prompt = StructuredChatAgent::default_prompt(&tools);

        assert!(prompt.contains("calculator"));
        assert!(prompt.contains("Calculate math expressions"));
        assert!(prompt.contains("search"));
        assert!(prompt.contains("Search the web"));
    }

    #[test]
    fn test_default_prompt_contains_format_instructions() {
        let tools = create_tools();
        let prompt = StructuredChatAgent::default_prompt(&tools);

        assert!(prompt.contains("action"));
        assert!(prompt.contains("action_input"));
        assert!(prompt.contains("Final Answer"));
        assert!(prompt.contains("$JSON_BLOB"));
    }

    #[test]
    fn test_default_prompt_lists_tool_names() {
        let tools = create_tools();
        let prompt = StructuredChatAgent::default_prompt(&tools);

        // Should list tool names as valid action values
        assert!(prompt.contains("calculator, search"));
    }

    #[test]
    fn test_default_prompt_empty_tools() {
        let tools: Vec<Arc<dyn Tool>> = vec![];
        let prompt = StructuredChatAgent::default_prompt(&tools);

        // Should still generate a valid prompt structure
        assert!(prompt.contains("Final Answer"));
        assert!(prompt.contains("action"));
    }

    #[test]
    fn test_default_prompt_contains_example_format() {
        let tools = create_tools();
        let prompt = StructuredChatAgent::default_prompt(&tools);

        // Should show the JSON format examples
        assert!(prompt.contains("```"));
        assert!(prompt.contains("$TOOL_NAME"));
        assert!(prompt.contains("$INPUT"));
    }

    // ============================================================================
    // format_scratchpad Tests
    // ============================================================================

    #[test]
    fn test_format_scratchpad_empty() {
        let agent = create_test_agent();
        let result = agent.format_scratchpad(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_scratchpad_single_step() {
        let agent = create_test_agent();
        let step = AgentStep {
            action: AgentAction::new("calculator", ToolInput::from("2+2"), "Need to calculate"),
            observation: "4".to_string(),
        };

        let result = agent.format_scratchpad(&[step]);

        assert!(result.contains("Thought: Need to calculate"));
        assert!(result.contains("Observation: 4"));
        assert!(result.contains("previous work"));
    }

    #[test]
    fn test_format_scratchpad_multiple_steps() {
        let agent = create_test_agent();
        let steps = vec![
            AgentStep {
                action: AgentAction::new("calculator", ToolInput::from("5*5"), "First step"),
                observation: "25".to_string(),
            },
            AgentStep {
                action: AgentAction::new("search", ToolInput::from("query"), "Second step"),
                observation: "Found results".to_string(),
            },
        ];

        let result = agent.format_scratchpad(&steps);

        assert!(result.contains("First step"));
        assert!(result.contains("Observation: 25"));
        assert!(result.contains("Second step"));
        assert!(result.contains("Observation: Found results"));
    }

    #[test]
    fn test_format_scratchpad_preserves_order() {
        let agent = create_test_agent();
        let steps = vec![
            AgentStep {
                action: AgentAction::new("tool", ToolInput::from("a"), "Step A"),
                observation: "Result A".to_string(),
            },
            AgentStep {
                action: AgentAction::new("tool", ToolInput::from("b"), "Step B"),
                observation: "Result B".to_string(),
            },
            AgentStep {
                action: AgentAction::new("tool", ToolInput::from("c"), "Step C"),
                observation: "Result C".to_string(),
            },
        ];

        let result = agent.format_scratchpad(&steps);

        // Check order by finding positions
        let pos_a = result.find("Step A").unwrap();
        let pos_b = result.find("Step B").unwrap();
        let pos_c = result.find("Step C").unwrap();

        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
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
    fn test_parse_output_final_answer_json_value() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action": "Final Answer",
  "action_input": {"result": 42, "status": "success"}
}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Finish(finish) => {
                // Non-string action_input should be stringified
                assert!(finish.output.contains("42"));
                assert!(finish.output.contains("success"));
            }
            _ => panic!("Expected AgentFinish"),
        }
    }

    #[test]
    fn test_parse_output_final_answer_no_input() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action": "Final Answer"
}
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
  "action_input": "2 + 2"
}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "calculator");
                match action.tool_input {
                    ToolInput::String(s) => assert_eq!(s, "2 + 2"),
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
  "action_input": {"query": "rust programming", "limit": 10}
}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "search");
                match action.tool_input {
                    ToolInput::Structured(v) => {
                        assert_eq!(v["query"], "rust programming");
                        assert_eq!(v["limit"], 10);
                    }
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
  "action": "calculator"
}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "calculator");
                // Missing action_input should default to empty object
                match action.tool_input {
                    ToolInput::Structured(v) => assert!(v.as_object().unwrap().is_empty()),
                    _ => panic!("Expected structured input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    // ============================================================================
    // parse_output Tests - JSON Formats
    // ============================================================================

    #[test]
    fn test_parse_output_json_without_language_specifier() {
        let agent = create_test_agent();
        let output = r#"```
{
  "action": "calculator",
  "action_input": "5 * 5"
}
```"#;

        let result = agent.parse_output(output).unwrap();
        assert!(result.is_action());
    }

    #[test]
    fn test_parse_output_json_with_surrounding_text() {
        let agent = create_test_agent();
        let output = r#"I need to calculate this.

```json
{
  "action": "calculator",
  "action_input": "10 / 2"
}
```

Let me know if you need more help."#;

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
        // Some LLMs return arrays, should take first element
        let output = r#"```json
[{
  "action": "search",
  "action_input": "query"
}]
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "search");
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_no_json_block() {
        let agent = create_test_agent();
        // If no JSON block found, treat entire text as final answer
        let output = "This is just a plain text response.";

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Finish(finish) => {
                assert_eq!(finish.output, output);
            }
            _ => panic!("Expected AgentFinish for non-JSON response"),
        }
    }

    // ============================================================================
    // parse_output Tests - Error Cases
    // ============================================================================

    #[test]
    fn test_parse_output_invalid_json() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action": "calculator"
  "action_input": "missing comma"
}
```"#;

        let result = agent.parse_output(output);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_output_missing_action_field() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action_input": "no action field"
}
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

    // ============================================================================
    // parse_output Tests - Edge Cases
    // ============================================================================

    #[test]
    fn test_parse_output_preserves_log() {
        let agent = create_test_agent();
        let output = r#"Let me think about this...

```json
{
  "action": "calculator",
  "action_input": "100"
}
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
    fn test_parse_output_unicode_content() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action": "search",
  "action_input": "Rust プログラミング言語"
}
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
    fn test_parse_output_whitespace_in_json() {
        let agent = create_test_agent();
        let output = r#"```json

{
  "action"  :  "calculator"  ,
  "action_input"  :  "1 + 1"
}

```"#;

        let result = agent.parse_output(output).unwrap();
        assert!(result.is_action());
    }

    #[test]
    fn test_parse_output_nested_json_in_input() {
        let agent = create_test_agent();
        let output = r#"```json
{
  "action": "api_call",
  "action_input": {
    "endpoint": "/users",
    "method": "POST",
    "body": {
      "name": "John",
      "age": 30
    }
  }
}
```"#;

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                match action.tool_input {
                    ToolInput::Structured(v) => {
                        assert_eq!(v["endpoint"], "/users");
                        assert_eq!(v["body"]["name"], "John");
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

        // Step 1: No previous steps
        let scratchpad = agent.format_scratchpad(&[]);
        assert!(scratchpad.is_empty());

        // Step 2: Parse tool action
        let llm_output = r#"```json
{"action": "calculator", "action_input": "5 + 5"}
```"#;
        let decision = agent.parse_output(llm_output).unwrap();
        let action = decision.as_action().unwrap();
        assert_eq!(action.tool, "calculator");

        // Step 3: Create step and format scratchpad
        let step = AgentStep {
            action: action.clone(),
            observation: "10".to_string(),
        };
        let scratchpad = agent.format_scratchpad(&[step]);
        assert!(scratchpad.contains("10"));

        // Step 4: Parse final answer
        let final_output = r#"```json
{"action": "Final Answer", "action_input": "The result is 10"}
```"#;
        let final_decision = agent.parse_output(final_output).unwrap();
        assert!(final_decision.is_finish());
    }
}
