use std::sync::OnceLock;

use crate::core::error::Result;

use super::{Agent, AgentAction, AgentDecision, AgentFinish, AgentStep};

/// `ReAct` (Reason + Act) agent that uses prompt-based reasoning
///
/// Unlike `ToolCallingAgent` which relies on native tool calling support,
/// `ReAct` agents work with any LLM by using carefully crafted prompts that
/// demonstrate the Thought-Action-Observation loop through few-shot examples.
///
/// # Pattern
///
/// The `ReAct` pattern (from <https://arxiv.org/abs/2210.03629>) interleaves:
/// 1. **Thought**: Agent reasons about what to do next
/// 2. **Action**: Agent decides on tool to use (format: `Action: ToolName[input]`)
/// 3. **Observation**: Tool output is provided back
/// 4. Loop continues until agent decides to finish: `Action: Finish[answer]`
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::agents::ReActAgent;
/// use dashflow::core::language_models::ChatModel;
/// use std::sync::Arc;
///
/// async fn example(llm: Arc<dyn ChatModel>) {
///     let tools = vec![/* tools */];
///
///     let agent = ReActAgent::new(
///         llm,
///         tools,
///         "Answer questions using available tools."
///     );
///
///     // Use with AgentExecutor
/// }
/// ```
///
/// # Comparison to `ToolCallingAgent`
///
/// **`ReAct` Agent (this)**:
/// - Works with any LLM (no tool calling API required)
/// - Relies on prompt engineering and text parsing
/// - Useful for LLMs without native tool support
/// - More transparent reasoning (thoughts visible in output)
///
/// **`ToolCallingAgent`**:
/// - Requires LLM with native tool calling (`OpenAI`, Anthropic, etc.)
/// - More reliable parsing (structured tool call objects)
/// - Generally faster and more accurate
pub struct ReActAgent {
    /// Language model for reasoning
    llm: std::sync::Arc<dyn crate::core::language_models::ChatModel>,
    /// Tools available to the agent
    tools: Vec<std::sync::Arc<dyn crate::core::tools::Tool>>,
    /// Tool definitions for passing to language model
    tool_definitions: Vec<crate::core::language_models::ToolDefinition>,
    /// System instructions
    system_prompt: String,
    /// Few-shot examples demonstrating the `ReAct` pattern
    examples: Vec<String>,
}

impl ReActAgent {
    /// Creates a new ReAct agent with default few-shot examples.
    ///
    /// The agent uses the Reasoning and Acting (ReAct) paradigm to interleave
    /// reasoning traces with tool calls, improving task completion accuracy.
    ///
    /// # Arguments
    ///
    /// * `llm` - The language model to use for reasoning
    /// * `tools` - Available tools the agent can call
    /// * `system_prompt` - Instructions defining the agent's behavior
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let agent = ReActAgent::new(
    ///     model,
    ///     vec![search_tool, calculator_tool],
    ///     "You are a helpful assistant that can search and calculate.",
    /// );
    /// ```
    pub fn new(
        llm: std::sync::Arc<dyn crate::core::language_models::ChatModel>,
        tools: Vec<std::sync::Arc<dyn crate::core::tools::Tool>>,
        system_prompt: impl Into<String>,
    ) -> Self {
        let examples = Self::default_examples();
        let tool_definitions = crate::core::tools::tools_to_definitions(&tools);
        Self {
            llm,
            tools,
            tool_definitions,
            system_prompt: system_prompt.into(),
            examples,
        }
    }

    /// Creates a new ReAct agent with custom few-shot examples.
    ///
    /// Use this when the default examples don't match your use case,
    /// or when you want to demonstrate specific reasoning patterns.
    ///
    /// # Arguments
    ///
    /// * `examples` - Custom few-shot examples showing the ReAct format
    pub fn with_examples(
        llm: std::sync::Arc<dyn crate::core::language_models::ChatModel>,
        tools: Vec<std::sync::Arc<dyn crate::core::tools::Tool>>,
        system_prompt: impl Into<String>,
        examples: Vec<String>,
    ) -> Self {
        let tool_definitions = crate::core::tools::tools_to_definitions(&tools);
        Self {
            llm,
            tools,
            tool_definitions,
            system_prompt: system_prompt.into(),
            examples,
        }
    }

    fn default_examples() -> Vec<String> {
        vec![
            r"Question: What is 15 multiplied by 23?
Thought: I need to multiply 15 by 23. I should use the calculator tool.
Action: calculator[15 * 23]
Observation: 345
Thought: The calculator returned 345, which is the answer.
Action: Finish[345]"
                .to_string(),
            r"Question: If I buy 5 apples at $2 each and 3 oranges at $3 each, what is my total?
Thought: I need to calculate the cost of apples and oranges separately, then add them.
Action: calculator[5 * 2]
Observation: 10
Thought: Apples cost $10. Now I need to calculate the cost of oranges.
Action: calculator[3 * 3]
Observation: 9
Thought: Oranges cost $9. Now I need to add the two amounts.
Action: calculator[10 + 9]
Observation: 19
Thought: The total cost is $19.
Action: Finish[$19]"
                .to_string(),
        ]
    }

    pub(super) fn build_prompt(&self, input: &str, intermediate_steps: &[AgentStep]) -> String {
        let mut prompt = String::new();

        prompt.push_str(&self.system_prompt);
        prompt.push_str("\n\n");

        prompt.push_str("You have access to the following tools:\n\n");
        for tool in &self.tools {
            prompt.push_str(&format!("- {}: {}\n", tool.name(), tool.description()));
        }
        prompt.push('\n');

        prompt.push_str("Use the following format:\n\n");
        prompt.push_str("Question: the input question you must answer\n");
        prompt.push_str("Thought: you should always think about what to do\n");
        prompt.push_str("Action: the action to take, should be one of [");
        for (i, tool) in self.tools.iter().enumerate() {
            if i > 0 {
                prompt.push_str(", ");
            }
            prompt.push_str(tool.name());
        }
        prompt.push_str("]\n");
        prompt.push_str("Action format: ToolName[input]\n");
        prompt.push_str("Observation: the result of the action\n");
        prompt.push_str("... (this Thought/Action/Observation can repeat N times)\n");
        prompt.push_str("Thought: I now know the final answer\n");
        prompt.push_str("Action: Finish[final answer here]\n\n");

        if !self.examples.is_empty() {
            prompt.push_str("Examples:\n\n");
            for (i, example) in self.examples.iter().enumerate() {
                prompt.push_str(&format!("Example {}:\n{}\n\n", i + 1, example));
            }
        }

        prompt.push_str(&format!("Question: {input}\n"));

        for step in intermediate_steps {
            prompt.push_str(&step.action.log);
            if !step.action.log.ends_with('\n') {
                prompt.push('\n');
            }

            let tool_input_str = match &step.action.tool_input {
                crate::core::tools::ToolInput::String(s) => s.clone(),
                crate::core::tools::ToolInput::Structured(v) => v.to_string(),
            };
            prompt.push_str(&format!(
                "Action: {}[{}]\n",
                step.action.tool, tool_input_str
            ));

            prompt.push_str(&format!("Observation: {}\n", step.observation));
        }

        prompt.push_str("Thought:");
        prompt
    }

    pub(super) fn parse_output(&self, text: &str) -> Result<AgentDecision> {
        use crate::core::error::Error;

        let lines: Vec<&str> = text.trim().split('\n').collect();

        let action_line = lines
            .iter()
            .rev()
            .find(|line| line.trim().starts_with("Action:"))
            .ok_or_else(|| {
                Error::InvalidInput(format!(
                    "Could not find 'Action:' in LLM output. Output:\n{text}"
                ))
            })?;

        let action_str = action_line
            .trim()
            .strip_prefix("Action:")
            .ok_or_else(|| Error::InvalidInput("Failed to strip 'Action:' prefix".to_string()))?
            .trim();

        static REACT_ACTION_REGEX: OnceLock<regex::Regex> = OnceLock::new();
        let re = REACT_ACTION_REGEX
            .get_or_init(|| regex::Regex::new(r"^(.*?)\[(.*?)\]$").expect("valid regex"));

        let captures = re.captures(action_str).ok_or_else(|| {
            Error::InvalidInput(format!(
                "Could not parse action format. Expected 'ToolName[input]', got: {action_str}"
            ))
        })?;

        let tool_name = captures
            .get(1)
            .ok_or_else(|| Error::InvalidInput("Missing tool name in capture group".to_string()))?
            .as_str()
            .trim();
        let tool_input = captures
            .get(2)
            .ok_or_else(|| Error::InvalidInput("Missing tool input in capture group".to_string()))?
            .as_str()
            .trim();

        if tool_name.eq_ignore_ascii_case("finish") {
            return Ok(AgentDecision::Finish(AgentFinish::new(
                tool_input,
                text.to_string(),
            )));
        }

        let tool_exists = self.tools.iter().any(|t| t.name() == tool_name);
        if !tool_exists {
            return Err(Error::InvalidInput(format!(
                "Unknown tool: {}. Available tools: {:?}",
                tool_name,
                self.tools.iter().map(|t| t.name()).collect::<Vec<_>>()
            )));
        }

        Ok(AgentDecision::Action(AgentAction::new(
            tool_name,
            crate::core::tools::ToolInput::String(tool_input.to_string()),
            text.to_string(),
        )))
    }
}

#[async_trait::async_trait]
impl Agent for ReActAgent {
    async fn plan(&self, input: &str, intermediate_steps: &[AgentStep]) -> Result<AgentDecision> {
        use crate::core::messages::{BaseMessage, Message};

        let prompt = self.build_prompt(input, intermediate_steps);

        let messages = vec![BaseMessage::from(Message::human(prompt))];
        let result = self
            .llm
            .generate(
                &messages,
                None,
                Some(&self.tool_definitions),
                Some(&crate::core::language_models::ToolChoice::Auto),
                None,
            )
            .await?;

        let text = result
            .generations
            .first()
            .ok_or_else(|| {
                crate::core::error::Error::OutputParsing(
                    "No generations returned from LLM".to_string(),
                )
            })?
            .message
            .content()
            .as_text();

        self.parse_output(&text)
    }
}

/// Type alias for zero-shot agent functionality.
///
/// This alias is deprecated in favor of using [`ReActAgent`] directly.
/// Zero-shot agents perform reasoning and acting in a single step without
/// requiring examples, making them suitable for straightforward tasks.
#[deprecated(
    since = "1.11.3",
    note = "Use ReActAgent directly. This alias will be removed in v2.0"
)]
pub type ZeroShotAgent = ReActAgent;

/// Type alias for MRKL (Modular Reasoning, Knowledge, and Language) agent.
///
/// This alias is deprecated in favor of using [`ReActAgent`] directly.
/// MRKL is an architecture that combines language models with modular
/// reasoning capabilities, which is now standard in ReActAgent.
#[deprecated(
    since = "1.11.3",
    note = "Use ReActAgent directly. This alias will be removed in v2.0"
)]
pub type MRKLAgent = ReActAgent;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tools::{Tool, ToolInput};
    use std::sync::Arc;

    /// Mock tool for testing ReActAgent
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

    fn create_test_agent() -> ReActAgent {
        let llm: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(MockTool::new("calculator", "Performs mathematical calculations")),
            Arc::new(MockTool::new("search", "Searches for information")),
        ];
        ReActAgent::new(llm, tools, "You are a helpful assistant.")
    }

    fn create_test_agent_with_single_tool() -> ReActAgent {
        let llm: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MockTool::new(
            "calculator",
            "Performs mathematical calculations",
        ))];
        ReActAgent::new(llm, tools, "You are a helpful assistant.")
    }

    // ============================================================================
    // Constructor Tests
    // ============================================================================

    #[test]
    fn test_react_agent_new() {
        let agent = create_test_agent();
        assert_eq!(agent.system_prompt, "You are a helpful assistant.");
        assert_eq!(agent.tools.len(), 2);
        assert_eq!(agent.tool_definitions.len(), 2);
        assert!(!agent.examples.is_empty());
    }

    #[test]
    fn test_react_agent_new_with_string_prompt() {
        let llm: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn Tool>> = vec![];
        let agent = ReActAgent::new(llm, tools, String::from("Custom prompt"));
        assert_eq!(agent.system_prompt, "Custom prompt");
    }

    #[test]
    fn test_react_agent_with_examples() {
        let llm: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MockTool::new("tool1", "description"))];
        let custom_examples = vec!["Example 1".to_string(), "Example 2".to_string()];

        let agent = ReActAgent::with_examples(
            llm,
            tools,
            "System prompt",
            custom_examples.clone(),
        );

        assert_eq!(agent.examples.len(), 2);
        assert_eq!(agent.examples[0], "Example 1");
        assert_eq!(agent.examples[1], "Example 2");
    }

    #[test]
    fn test_react_agent_with_empty_examples() {
        let llm: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn Tool>> = vec![];
        let agent = ReActAgent::with_examples(llm, tools, "Prompt", vec![]);
        assert!(agent.examples.is_empty());
    }

    #[test]
    fn test_react_agent_default_examples() {
        let examples = ReActAgent::default_examples();
        assert_eq!(examples.len(), 2);
        // First example should be about multiplication
        assert!(examples[0].contains("15 multiplied by 23"));
        assert!(examples[0].contains("calculator"));
        // Second example should be about buying items
        assert!(examples[1].contains("apples"));
        assert!(examples[1].contains("oranges"));
    }

    // ============================================================================
    // build_prompt Tests
    // ============================================================================

    #[test]
    fn test_build_prompt_basic() {
        let agent = create_test_agent();
        let prompt = agent.build_prompt("What is 2+2?", &[]);

        assert!(prompt.contains("You are a helpful assistant."));
        assert!(prompt.contains("calculator"));
        assert!(prompt.contains("search"));
        assert!(prompt.contains("What is 2+2?"));
        assert!(prompt.ends_with("Thought:"));
    }

    #[test]
    fn test_build_prompt_contains_tool_descriptions() {
        let agent = create_test_agent();
        let prompt = agent.build_prompt("test input", &[]);

        assert!(prompt.contains("calculator: Performs mathematical calculations"));
        assert!(prompt.contains("search: Searches for information"));
    }

    #[test]
    fn test_build_prompt_contains_format_instructions() {
        let agent = create_test_agent();
        let prompt = agent.build_prompt("test", &[]);

        assert!(prompt.contains("Question:"));
        assert!(prompt.contains("Thought:"));
        assert!(prompt.contains("Action:"));
        assert!(prompt.contains("Observation:"));
        assert!(prompt.contains("Finish[final answer here]"));
    }

    #[test]
    fn test_build_prompt_contains_tool_list() {
        let agent = create_test_agent();
        let prompt = agent.build_prompt("test", &[]);

        // Should list tools in brackets
        assert!(prompt.contains("[calculator, search]"));
    }

    #[test]
    fn test_build_prompt_contains_examples() {
        let agent = create_test_agent();
        let prompt = agent.build_prompt("test", &[]);

        assert!(prompt.contains("Examples:"));
        assert!(prompt.contains("Example 1:"));
        assert!(prompt.contains("Example 2:"));
    }

    #[test]
    fn test_build_prompt_no_examples_when_empty() {
        let llm: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MockTool::new("tool", "desc"))];
        let agent = ReActAgent::with_examples(llm, tools, "Prompt", vec![]);

        let prompt = agent.build_prompt("test", &[]);
        assert!(!prompt.contains("Examples:"));
    }

    #[test]
    fn test_build_prompt_with_intermediate_steps() {
        let agent = create_test_agent();
        let step = AgentStep {
            action: AgentAction::new(
                "calculator",
                ToolInput::String("5 * 5".to_string()),
                "I need to multiply 5 by 5",
            ),
            observation: "25".to_string(),
        };

        let prompt = agent.build_prompt("What is 5 * 5?", &[step]);

        assert!(prompt.contains("I need to multiply 5 by 5"));
        assert!(prompt.contains("Action: calculator[5 * 5]"));
        assert!(prompt.contains("Observation: 25"));
    }

    #[test]
    fn test_build_prompt_with_multiple_intermediate_steps() {
        let agent = create_test_agent();
        let steps = vec![
            AgentStep {
                action: AgentAction::new(
                    "calculator",
                    ToolInput::String("10 + 5".to_string()),
                    "Adding 10 and 5",
                ),
                observation: "15".to_string(),
            },
            AgentStep {
                action: AgentAction::new(
                    "calculator",
                    ToolInput::String("15 * 2".to_string()),
                    "Multiplying by 2",
                ),
                observation: "30".to_string(),
            },
        ];

        let prompt = agent.build_prompt("Calculate (10+5)*2", &steps);

        // Both steps should appear in order
        assert!(prompt.contains("Adding 10 and 5"));
        assert!(prompt.contains("Observation: 15"));
        assert!(prompt.contains("Multiplying by 2"));
        assert!(prompt.contains("Observation: 30"));
    }

    #[test]
    fn test_build_prompt_with_structured_tool_input() {
        let agent = create_test_agent();
        let step = AgentStep {
            action: AgentAction::new(
                "search",
                ToolInput::Structured(serde_json::json!({"query": "rust programming"})),
                "Searching for information",
            ),
            observation: "Found results".to_string(),
        };

        let prompt = agent.build_prompt("Search for rust", &[step]);

        // Structured input should be JSON stringified
        assert!(prompt.contains("Action: search["));
        assert!(prompt.contains("query"));
    }

    #[test]
    fn test_build_prompt_single_tool() {
        let agent = create_test_agent_with_single_tool();
        let prompt = agent.build_prompt("test", &[]);

        // Should only list one tool
        assert!(prompt.contains("[calculator]"));
        assert!(!prompt.contains("search"));
    }

    // ============================================================================
    // parse_output Tests
    // ============================================================================

    #[test]
    fn test_parse_output_finish_action() {
        let agent = create_test_agent();
        let output = "Thought: I know the answer now.\nAction: Finish[42]";

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Finish(finish) => {
                assert_eq!(finish.output, "42");
                assert_eq!(finish.log, output);
            }
            _ => panic!("Expected AgentFinish"),
        }
    }

    #[test]
    fn test_parse_output_finish_case_insensitive() {
        let agent = create_test_agent();
        let output = "Thought: Done.\nAction: FINISH[The answer is 42]";

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Finish(finish) => {
                assert_eq!(finish.output, "The answer is 42");
            }
            _ => panic!("Expected AgentFinish"),
        }
    }

    #[test]
    fn test_parse_output_tool_action() {
        let agent = create_test_agent();
        let output = "Thought: I need to calculate.\nAction: calculator[2 + 2]";

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
    fn test_parse_output_search_tool() {
        let agent = create_test_agent();
        let output = "Thought: Need more info.\nAction: search[rust programming language]";

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "search");
                match action.tool_input {
                    ToolInput::String(s) => assert_eq!(s, "rust programming language"),
                    _ => panic!("Expected string input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_unknown_tool_error() {
        let agent = create_test_agent();
        let output = "Thought: Using unknown tool.\nAction: unknown_tool[input]";

        let result = agent.parse_output(output);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Unknown tool"));
        assert!(err.to_string().contains("unknown_tool"));
    }

    #[test]
    fn test_parse_output_no_action_error() {
        let agent = create_test_agent();
        let output = "Thought: Just thinking, no action.";

        let result = agent.parse_output(output);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Action:"));
    }

    #[test]
    fn test_parse_output_invalid_format_error() {
        let agent = create_test_agent();
        let output = "Thought: Bad format.\nAction: calculator without brackets";

        let result = agent.parse_output(output);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("ToolName[input]"));
    }

    #[test]
    fn test_parse_output_uses_last_action() {
        let agent = create_test_agent();
        let output = "Thought: First thought.\nAction: search[first query]\nObservation: result\nThought: Second thought.\nAction: calculator[2+2]";

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                // Should use the LAST action line
                assert_eq!(action.tool, "calculator");
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_whitespace_handling() {
        let agent = create_test_agent();
        let output = "  Thought: With whitespace.  \n  Action: calculator[ 5 + 5 ]  ";

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "calculator");
                match action.tool_input {
                    ToolInput::String(s) => assert_eq!(s, "5 + 5"),
                    _ => panic!("Expected string input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_empty_input() {
        let agent = create_test_agent();
        let output = "Thought: Empty input.\nAction: calculator[]";

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                match action.tool_input {
                    ToolInput::String(s) => assert_eq!(s, ""),
                    _ => panic!("Expected string input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_preserves_log() {
        let agent = create_test_agent();
        let output = "Thought: I need to calculate something complex.\nAction: calculator[100 / 5]";

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.log, output);
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_multiline_thought() {
        let agent = create_test_agent();
        let output = "Thought: This is a complex problem.\nI need to think about it carefully.\nLet me consider the options.\nAction: calculator[1+1]";

        let result = agent.parse_output(output).unwrap();
        assert!(result.is_action());
    }

    #[test]
    fn test_parse_output_special_characters_in_input() {
        let agent = create_test_agent();
        let output = "Thought: Special chars.\nAction: search[rust \"async/await\" & tokio]";

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                match action.tool_input {
                    ToolInput::String(s) => {
                        assert!(s.contains("async/await"));
                        assert!(s.contains("&"));
                    }
                    _ => panic!("Expected string input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_unicode_in_input() {
        let agent = create_test_agent();
        let output = "Thought: Unicode test.\nAction: search[Rust プログラミング 言語]";

        let result = agent.parse_output(output).unwrap();
        match result {
            AgentDecision::Action(action) => {
                match action.tool_input {
                    ToolInput::String(s) => {
                        assert!(s.contains("プログラミング"));
                    }
                    _ => panic!("Expected string input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    // ============================================================================
    // Integration Tests
    // ============================================================================

    #[test]
    fn test_agent_workflow_simple_calculation() {
        let agent = create_test_agent();

        // Step 1: Initial prompt
        let prompt1 = agent.build_prompt("What is 10 + 5?", &[]);
        assert!(prompt1.contains("What is 10 + 5?"));

        // Step 2: Parse LLM output (simulated)
        let llm_output = "Thought: I need to add 10 and 5.\nAction: calculator[10 + 5]";
        let decision1 = agent.parse_output(llm_output).unwrap();
        assert!(decision1.is_action());

        // Step 3: Create step and continue
        let action = decision1.as_action().unwrap();
        let step = AgentStep {
            action: action.clone(),
            observation: "15".to_string(),
        };

        // Step 4: Build next prompt with history
        let prompt2 = agent.build_prompt("What is 10 + 5?", &[step]);
        assert!(prompt2.contains("Observation: 15"));

        // Step 5: Parse final answer
        let final_output = "Thought: The answer is 15.\nAction: Finish[15]";
        let decision2 = agent.parse_output(final_output).unwrap();
        assert!(decision2.is_finish());

        let finish = decision2.as_finish().unwrap();
        assert_eq!(finish.output, "15");
    }

    #[test]
    fn test_agent_workflow_multi_step() {
        let agent = create_test_agent();

        // Multiple steps building up
        let step1 = AgentStep {
            action: AgentAction::new("calculator", ToolInput::from("5 * 2"), "First calc"),
            observation: "10".to_string(),
        };

        let step2 = AgentStep {
            action: AgentAction::new("calculator", ToolInput::from("10 + 3"), "Second calc"),
            observation: "13".to_string(),
        };

        let prompt = agent.build_prompt("What is (5*2)+3?", &[step1, step2]);

        // All history should be present
        assert!(prompt.contains("First calc"));
        assert!(prompt.contains("Observation: 10"));
        assert!(prompt.contains("Second calc"));
        assert!(prompt.contains("Observation: 13"));
    }

    // ============================================================================
    // Deprecated Alias Tests
    // ============================================================================

    #[test]
    #[allow(deprecated)]
    fn test_zero_shot_agent_alias() {
        // Just verify the type alias exists and compiles
        fn _accepts_zero_shot(_: ZeroShotAgent) {}
    }

    #[test]
    #[allow(deprecated)]
    fn test_mrkl_agent_alias() {
        // Just verify the type alias exists and compiles
        fn _accepts_mrkl(_: MRKLAgent) {}
    }
}
