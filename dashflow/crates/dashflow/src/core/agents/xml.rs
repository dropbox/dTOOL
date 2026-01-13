//! XML agent implementation.

use crate::core::error::Result;
use crate::core::tools::ToolInput;

use super::{Agent, AgentAction, AgentDecision, AgentFinish, AgentStep};

/// XML Agent - uses XML tags for tool invocation and final answers.
///
/// This agent format is particularly effective with models that work well with
/// structured XML markup (like Claude models). It uses:
/// - `<tool>name</tool>` for tool selection
/// - `<tool_input>input</tool_input>` for tool input
/// - `<observation>result</observation>` for tool results
/// - `<final_answer>answer</final_answer>` for final responses
///
/// Key features:
/// - XML escaping support for tool names/inputs containing XML tags
/// - Minimal escaping format: replaces `<tool>` with `[[tool]]` in content
/// - Stop sequences to prevent hallucination of observations
///
/// Python baseline: `dashflow_classic/agents/xml/base.py`
pub struct XmlAgent {
    chat_model: std::sync::Arc<dyn crate::core::language_models::ChatModel>,
    tool_definitions: Vec<crate::core::language_models::ToolDefinition>,
    system_prompt: String,
    escape_format: bool, // true = "minimal", false = None
    stop_sequences: Vec<String>,
}

impl XmlAgent {
    /// Create a new XML Agent with the given chat model and tools.
    pub fn new(
        chat_model: std::sync::Arc<dyn crate::core::language_models::ChatModel>,
        tools: &[std::sync::Arc<dyn crate::core::tools::Tool>],
    ) -> Self {
        let system_prompt = Self::default_prompt(tools);
        let stop_sequences = vec!["</tool_input>".to_string()];
        let tools_vec: Vec<std::sync::Arc<dyn crate::core::tools::Tool>> = tools.to_vec();
        let tool_definitions = crate::core::tools::tools_to_definitions(&tools_vec);
        Self {
            chat_model,
            tool_definitions,
            system_prompt,
            escape_format: true, // Use minimal escaping by default
            stop_sequences,
        }
    }

    /// Create agent with custom prompt and stop sequences.
    pub fn with_custom_prompt(
        chat_model: std::sync::Arc<dyn crate::core::language_models::ChatModel>,
        tools: Vec<std::sync::Arc<dyn crate::core::tools::Tool>>,
        system_prompt: String,
        escape_format: bool,
        stop_sequences: Vec<String>,
    ) -> Self {
        let tool_definitions = crate::core::tools::tools_to_definitions(&tools);
        Self {
            chat_model,
            tool_definitions,
            system_prompt,
            escape_format,
            stop_sequences,
        }
    }

    /// Generate the default system prompt for XML Agent.
    ///
    /// Python baseline: `dashflow_classic/agents/xml/prompt.py`
    pub fn default_prompt(tools: &[std::sync::Arc<dyn crate::core::tools::Tool>]) -> String {
        let mut tool_descriptions = String::new();
        for tool in tools {
            tool_descriptions.push_str(&format!("{}: {}\n", tool.name(), tool.description()));
        }

        format!(
            r"You are a helpful assistant. Help the user answer any questions.

You have access to the following tools:

{}

In order to use a tool, you can use <tool></tool> and <tool_input></tool_input> tags. You will then get back a response in the form <observation></observation>
For example, if you have a tool called 'search' that could run a google search, in order to search for the weather in SF you would respond:

<tool>search</tool><tool_input>weather in SF</tool_input>
<observation>64 degrees</observation>

When you are done, respond with a final answer between <final_answer></final_answer>. For example:

<final_answer>The weather in SF is 64 degrees</final_answer>

Begin!",
            tool_descriptions.trim_end()
        )
    }

    /// Escape XML tags in text to prevent parsing conflicts.
    ///
    /// Python baseline: `dashflow_classic/agents/format_scratchpad/xml.py:_escape()`
    #[must_use]
    pub fn escape_xml(text: &str) -> String {
        text.replace("<tool>", "[[tool]]")
            .replace("</tool>", "[[/tool]]")
            .replace("<tool_input>", "[[tool_input]]")
            .replace("</tool_input>", "[[/tool_input]]")
            .replace("<observation>", "[[observation]]")
            .replace("</observation>", "[[/observation]]")
    }

    /// Unescape XML tags back to original form.
    ///
    /// Python baseline: `dashflow_classic/agents/output_parsers/xml.py:_unescape()`
    #[must_use]
    pub fn unescape_xml(text: &str) -> String {
        text.replace("[[tool]]", "<tool>")
            .replace("[[/tool]]", "</tool>")
            .replace("[[tool_input]]", "<tool_input>")
            .replace("[[/tool_input]]", "</tool_input>")
            .replace("[[observation]]", "<observation>")
            .replace("[[/observation]]", "</observation>")
    }

    /// Convert `ToolInput` to string representation
    ///
    /// Matches Python's `str(action.tool_input)` behavior
    fn tool_input_to_string(tool_input: &ToolInput) -> String {
        match tool_input {
            ToolInput::String(s) => s.clone(),
            ToolInput::Structured(v) => v.to_string(),
        }
    }

    /// Format intermediate steps as XML string.
    ///
    /// Python baseline: `dashflow_classic/agents/format_scratchpad/xml.py:format_xml()`
    #[must_use]
    pub fn format_scratchpad(&self, intermediate_steps: &[AgentStep]) -> String {
        let mut log = String::new();
        for step in intermediate_steps {
            // Convert tool_input to string
            let tool_input_str = Self::tool_input_to_string(&step.action.tool_input);

            let tool = if self.escape_format {
                Self::escape_xml(&step.action.tool)
            } else {
                step.action.tool.clone()
            };

            let tool_input = if self.escape_format {
                Self::escape_xml(&tool_input_str)
            } else {
                tool_input_str
            };

            let observation = if self.escape_format {
                Self::escape_xml(&step.observation)
            } else {
                step.observation.clone()
            };

            log.push_str(&format!(
                "<tool>{tool}</tool><tool_input>{tool_input}</tool_input><observation>{observation}</observation>"
            ));
        }
        log
    }

    /// Parse XML-formatted output into `AgentDecision`.
    ///
    /// Python baseline: `dashflow_classic/agents/output_parsers/xml.py:XMLAgentOutputParser.parse()`
    pub fn parse_output(&self, text: &str) -> Result<AgentDecision> {
        use regex::Regex;

        // Check for tool invocation first
        let tool_regex = Regex::new(r"<tool>(.*?)</tool>").unwrap();
        let tool_matches: Vec<_> = tool_regex.captures_iter(text).collect();

        if !tool_matches.is_empty() {
            // Python baseline: Expected exactly one <tool> block
            if tool_matches.len() != 1 {
                return Err(crate::core::error::Error::Agent(format!(
                    "Malformed tool invocation: expected exactly one <tool> block, but found {}",
                    tool_matches.len()
                )));
            }

            let mut tool = tool_matches[0][1].to_string();

            // Match optional tool input
            let input_regex = Regex::new(r"<tool_input>(.*?)</tool_input>").unwrap();
            let input_matches: Vec<_> = input_regex.captures_iter(text).collect();

            // Python baseline: Expected at most one <tool_input> block
            if input_matches.len() > 1 {
                return Err(crate::core::error::Error::Agent(format!(
                    "Malformed tool invocation: expected at most one <tool_input> block, but found {}",
                    input_matches.len()
                )));
            }

            let mut tool_input = if input_matches.is_empty() {
                String::new()
            } else {
                input_matches[0][1].to_string()
            };

            // Unescape if minimal escape format is used
            if self.escape_format {
                tool = Self::unescape_xml(&tool);
                tool_input = Self::unescape_xml(&tool_input);
            }

            return Ok(AgentDecision::Action(AgentAction {
                tool,
                tool_input: ToolInput::String(tool_input),
                log: text.to_string(),
            }));
        }

        // Check for final answer
        if text.contains("<final_answer>") && text.contains("</final_answer>") {
            let answer_regex = Regex::new(r"<final_answer>(.*?)</final_answer>").unwrap();
            let matches: Vec<_> = answer_regex.captures_iter(text).collect();

            // Python baseline: Expected exactly one <final_answer> block
            if matches.len() != 1 {
                return Err(crate::core::error::Error::Agent(
                    "Malformed output: expected exactly one <final_answer>...</final_answer> block"
                        .to_string(),
                ));
            }

            let mut answer = matches[0][1].to_string();

            // Unescape custom delimiters in final answer
            if self.escape_format {
                answer = Self::unescape_xml(&answer);
            }

            return Ok(AgentDecision::Finish(AgentFinish {
                output: answer,
                log: text.to_string(),
            }));
        }

        // No valid XML format found
        Err(crate::core::error::Error::Agent(
            "Malformed output: expected either a tool invocation or a final answer in XML format"
                .to_string(),
        ))
    }

    #[cfg(test)]
    pub(super) fn input_keys(&self) -> Vec<String> {
        vec!["input".to_string()]
    }

    #[cfg(test)]
    pub(super) fn output_keys(&self) -> Vec<String> {
        vec!["output".to_string()]
    }
}

#[async_trait::async_trait]
impl Agent for XmlAgent {
    async fn plan(&self, input: &str, intermediate_steps: &[AgentStep]) -> Result<AgentDecision> {
        use crate::core::messages::{BaseMessage, Message};

        // Format scratchpad as XML
        let scratchpad = self.format_scratchpad(intermediate_steps);

        // Build user message with scratchpad and input
        let user_content = if scratchpad.is_empty() {
            format!("\nQuestion: {input}")
        } else {
            format!("\nQuestion: {input}\n{scratchpad}")
        };

        let messages = vec![
            BaseMessage::from(Message::system(self.system_prompt.clone())),
            BaseMessage::from(Message::human(user_content)),
        ];

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

    // ==========================================================================
    // escape_xml tests
    // ==========================================================================

    #[test]
    fn test_escape_xml_empty_string() {
        assert_eq!(XmlAgent::escape_xml(""), "");
    }

    #[test]
    fn test_escape_xml_no_tags() {
        let input = "Hello, this is regular text without any XML tags.";
        assert_eq!(XmlAgent::escape_xml(input), input);
    }

    #[test]
    fn test_escape_xml_tool_tag() {
        assert_eq!(XmlAgent::escape_xml("<tool>"), "[[tool]]");
        assert_eq!(XmlAgent::escape_xml("</tool>"), "[[/tool]]");
    }

    #[test]
    fn test_escape_xml_tool_input_tag() {
        assert_eq!(XmlAgent::escape_xml("<tool_input>"), "[[tool_input]]");
        assert_eq!(XmlAgent::escape_xml("</tool_input>"), "[[/tool_input]]");
    }

    #[test]
    fn test_escape_xml_observation_tag() {
        assert_eq!(XmlAgent::escape_xml("<observation>"), "[[observation]]");
        assert_eq!(XmlAgent::escape_xml("</observation>"), "[[/observation]]");
    }

    #[test]
    fn test_escape_xml_mixed_content() {
        let input = "Use <tool>search</tool> with <tool_input>query</tool_input>";
        let expected = "Use [[tool]]search[[/tool]] with [[tool_input]]query[[/tool_input]]";
        assert_eq!(XmlAgent::escape_xml(input), expected);
    }

    #[test]
    fn test_escape_xml_nested_tags() {
        let input = "<tool><tool>inner</tool></tool>";
        let expected = "[[tool]][[tool]]inner[[/tool]][[/tool]]";
        assert_eq!(XmlAgent::escape_xml(input), expected);
    }

    // ==========================================================================
    // unescape_xml tests
    // ==========================================================================

    #[test]
    fn test_unescape_xml_empty_string() {
        assert_eq!(XmlAgent::unescape_xml(""), "");
    }

    #[test]
    fn test_unescape_xml_no_escaped_tags() {
        let input = "Hello, regular text without escaped tags.";
        assert_eq!(XmlAgent::unescape_xml(input), input);
    }

    #[test]
    fn test_unescape_xml_tool_tag() {
        assert_eq!(XmlAgent::unescape_xml("[[tool]]"), "<tool>");
        assert_eq!(XmlAgent::unescape_xml("[[/tool]]"), "</tool>");
    }

    #[test]
    fn test_unescape_xml_tool_input_tag() {
        assert_eq!(XmlAgent::unescape_xml("[[tool_input]]"), "<tool_input>");
        assert_eq!(XmlAgent::unescape_xml("[[/tool_input]]"), "</tool_input>");
    }

    #[test]
    fn test_unescape_xml_observation_tag() {
        assert_eq!(XmlAgent::unescape_xml("[[observation]]"), "<observation>");
        assert_eq!(XmlAgent::unescape_xml("[[/observation]]"), "</observation>");
    }

    #[test]
    fn test_unescape_xml_mixed_content() {
        let input = "Use [[tool]]search[[/tool]] with [[tool_input]]query[[/tool_input]]";
        let expected = "Use <tool>search</tool> with <tool_input>query</tool_input>";
        assert_eq!(XmlAgent::unescape_xml(input), expected);
    }

    // ==========================================================================
    // escape/unescape roundtrip tests
    // ==========================================================================

    #[test]
    fn test_escape_unescape_roundtrip() {
        let original = "Use <tool>search</tool> with <tool_input>weather SF</tool_input>";
        let escaped = XmlAgent::escape_xml(original);
        let unescaped = XmlAgent::unescape_xml(&escaped);
        assert_eq!(unescaped, original);
    }

    #[test]
    fn test_escape_unescape_roundtrip_all_tags() {
        let original =
            "<tool>t</tool><tool_input>i</tool_input><observation>o</observation>";
        let escaped = XmlAgent::escape_xml(original);
        assert!(!escaped.contains('<'));
        assert!(!escaped.contains('>'));
        let unescaped = XmlAgent::unescape_xml(&escaped);
        assert_eq!(unescaped, original);
    }

    // ==========================================================================
    // tool_input_to_string tests
    // ==========================================================================

    #[test]
    fn test_tool_input_to_string_with_string() {
        let input = ToolInput::String("hello world".to_string());
        assert_eq!(XmlAgent::tool_input_to_string(&input), "hello world");
    }

    #[test]
    fn test_tool_input_to_string_with_structured() {
        let input = ToolInput::Structured(serde_json::json!({"key": "value"}));
        let result = XmlAgent::tool_input_to_string(&input);
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }

    #[test]
    fn test_tool_input_to_string_with_empty_string() {
        let input = ToolInput::String(String::new());
        assert_eq!(XmlAgent::tool_input_to_string(&input), "");
    }

    // ==========================================================================
    // parse_output tests - tool invocation
    // ==========================================================================

    #[test]
    fn test_parse_output_tool_invocation() {
        let agent = create_test_agent();
        let text = "<tool>search</tool><tool_input>weather in SF</tool_input>";
        let result = agent.parse_output(text).unwrap();

        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "search");
                match action.tool_input {
                    ToolInput::String(s) => assert_eq!(s, "weather in SF"),
                    _ => panic!("Expected string tool input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_tool_without_input() {
        let agent = create_test_agent();
        let text = "<tool>get_time</tool>";
        let result = agent.parse_output(text).unwrap();

        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "get_time");
                match action.tool_input {
                    ToolInput::String(s) => assert_eq!(s, ""),
                    _ => panic!("Expected string tool input"),
                }
            }
            _ => panic!("Expected AgentAction"),
        }
    }

    #[test]
    fn test_parse_output_multiple_tools_error() {
        let agent = create_test_agent();
        let text = "<tool>search</tool><tool>calculate</tool>";
        let result = agent.parse_output(text);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("expected exactly one <tool> block"));
    }

    #[test]
    fn test_parse_output_multiple_tool_inputs_error() {
        let agent = create_test_agent();
        let text = "<tool>search</tool><tool_input>a</tool_input><tool_input>b</tool_input>";
        let result = agent.parse_output(text);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("expected at most one <tool_input> block"));
    }

    // ==========================================================================
    // parse_output tests - final answer
    // ==========================================================================

    #[test]
    fn test_parse_output_final_answer() {
        let agent = create_test_agent();
        let text = "<final_answer>The weather in SF is 64 degrees</final_answer>";
        let result = agent.parse_output(text).unwrap();

        match result {
            AgentDecision::Finish(finish) => {
                assert_eq!(finish.output, "The weather in SF is 64 degrees");
            }
            _ => panic!("Expected AgentFinish"),
        }
    }

    #[test]
    fn test_parse_output_final_answer_with_unescaping() {
        let agent = create_test_agent();
        // Content with escaped XML tags
        let text = "<final_answer>Use [[tool]]search[[/tool]] for queries</final_answer>";
        let result = agent.parse_output(text).unwrap();

        match result {
            AgentDecision::Finish(finish) => {
                // Should unescape the content
                assert_eq!(finish.output, "Use <tool>search</tool> for queries");
            }
            _ => panic!("Expected AgentFinish"),
        }
    }

    #[test]
    fn test_parse_output_multiple_final_answers_error() {
        let agent = create_test_agent();
        let text = "<final_answer>a</final_answer><final_answer>b</final_answer>";
        let result = agent.parse_output(text);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("expected exactly one <final_answer>"));
    }

    // ==========================================================================
    // parse_output tests - error cases
    // ==========================================================================

    #[test]
    fn test_parse_output_no_valid_format() {
        let agent = create_test_agent();
        let text = "Just some random text without proper XML format";
        let result = agent.parse_output(text);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("expected either a tool invocation or a final answer"));
    }

    #[test]
    fn test_parse_output_empty_string() {
        let agent = create_test_agent();
        let result = agent.parse_output("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_output_incomplete_final_answer() {
        let agent = create_test_agent();
        // Only opening tag, no closing
        let text = "<final_answer>The answer is";
        let result = agent.parse_output(text);
        assert!(result.is_err());
    }

    // ==========================================================================
    // format_scratchpad tests
    // ==========================================================================

    #[test]
    fn test_format_scratchpad_empty() {
        let agent = create_test_agent();
        let steps: Vec<AgentStep> = vec![];
        let result = agent.format_scratchpad(&steps);
        assert_eq!(result, "");
    }

    #[test]
    fn test_format_scratchpad_single_step() {
        let agent = create_test_agent();
        let steps = vec![AgentStep {
            action: AgentAction {
                tool: "search".to_string(),
                tool_input: ToolInput::String("weather SF".to_string()),
                log: String::new(),
            },
            observation: "64 degrees".to_string(),
        }];
        let result = agent.format_scratchpad(&steps);
        assert!(result.contains("<tool>search</tool>"));
        assert!(result.contains("<tool_input>weather SF</tool_input>"));
        assert!(result.contains("<observation>64 degrees</observation>"));
    }

    #[test]
    fn test_format_scratchpad_with_escaping() {
        let agent = create_test_agent();
        // Tool name contains XML tags (edge case)
        let steps = vec![AgentStep {
            action: AgentAction {
                tool: "get<data>".to_string(), // Contains < and > but not our tags
                tool_input: ToolInput::String("input".to_string()),
                log: String::new(),
            },
            observation: "result".to_string(),
        }];
        let result = agent.format_scratchpad(&steps);
        // The tool name should be in the output (our escaping only affects specific tags)
        assert!(result.contains("get<data>"));
    }

    #[test]
    fn test_format_scratchpad_multiple_steps() {
        let agent = create_test_agent();
        let steps = vec![
            AgentStep {
                action: AgentAction {
                    tool: "search".to_string(),
                    tool_input: ToolInput::String("query1".to_string()),
                    log: String::new(),
                },
                observation: "result1".to_string(),
            },
            AgentStep {
                action: AgentAction {
                    tool: "calculate".to_string(),
                    tool_input: ToolInput::String("2+2".to_string()),
                    log: String::new(),
                },
                observation: "4".to_string(),
            },
        ];
        let result = agent.format_scratchpad(&steps);
        // Should contain both steps in order
        assert!(result.contains("search"));
        assert!(result.contains("query1"));
        assert!(result.contains("result1"));
        assert!(result.contains("calculate"));
        assert!(result.contains("2+2"));
        assert!(result.contains("4"));
        // First step should come before second
        let search_pos = result.find("search").unwrap();
        let calc_pos = result.find("calculate").unwrap();
        assert!(search_pos < calc_pos);
    }

    // ==========================================================================
    // default_prompt tests
    // ==========================================================================

    #[test]
    fn test_default_prompt_contains_tool_descriptions() {
        let tools = create_test_tools();
        let prompt = XmlAgent::default_prompt(&tools);

        assert!(prompt.contains("You are a helpful assistant"));
        assert!(prompt.contains("<tool>"));
        assert!(prompt.contains("<tool_input>"));
        assert!(prompt.contains("<final_answer>"));
    }

    #[test]
    fn test_default_prompt_empty_tools() {
        let tools: Vec<std::sync::Arc<dyn crate::core::tools::Tool>> = vec![];
        let prompt = XmlAgent::default_prompt(&tools);
        // Should still generate valid prompt structure
        assert!(prompt.contains("You are a helpful assistant"));
        assert!(prompt.contains("<tool>"));
    }

    // ==========================================================================
    // XmlAgent construction tests
    // ==========================================================================

    #[test]
    fn test_xml_agent_input_keys() {
        let agent = create_test_agent();
        let keys = agent.input_keys();
        assert_eq!(keys, vec!["input"]);
    }

    #[test]
    fn test_xml_agent_output_keys() {
        let agent = create_test_agent();
        let keys = agent.output_keys();
        assert_eq!(keys, vec!["output"]);
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

    fn create_test_agent() -> XmlAgent {
        use std::sync::Arc;

        let mock_model: Arc<dyn crate::core::language_models::ChatModel> = Arc::new(MockChatModel);
        let tools = create_test_tools();
        XmlAgent::new(mock_model, &tools)
    }

    fn create_test_tools() -> Vec<std::sync::Arc<dyn crate::core::tools::Tool>> {
        // Return empty vec - XmlAgent works with empty tools for unit tests
        vec![]
    }
}
