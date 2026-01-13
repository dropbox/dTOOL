use crate::core::error::Result;

use super::{Agent, AgentConfigError, AgentDecision, AgentFinish, AgentStep};

/// Self-Ask with Search Agent that decomposes complex questions into sub-questions
///
/// This agent implements the self-ask pattern where complex questions are broken down
/// into simpler sub-questions that can be answered individually (typically using a
/// search tool), and then composed together to answer the original question.
///
/// # Pattern
///
/// The self-ask pattern follows this structure:
/// 1. Agent receives a complex question
/// 2. Agent determines if follow-up questions are needed
/// 3. If yes, agent asks a follow-up question (calls "Intermediate Answer" tool)
/// 4. Tool returns intermediate answer
/// 5. Steps 3-4 repeat until all sub-questions are answered
/// 6. Agent provides final answer using all intermediate answers
///
/// # Example Output Format
///
/// ```text
/// Question: Who lived longer, Muhammad Ali or Alan Turing?
/// Are follow up questions needed here: Yes
/// Follow up: How old was Muhammad Ali when he died?
/// Intermediate answer: Muhammad Ali was 74 years old when he died.
/// Follow up: How old was Alan Turing when he died?
/// Intermediate answer: Alan Turing was 41 years old when he died.
/// So the final answer is: Muhammad Ali
/// ```
///
/// # Tool Requirements
///
/// This agent expects exactly one tool named "Intermediate Answer" which should
/// be a search tool (like Google Search, Wikipedia, etc.) that can answer factual
/// sub-questions.
pub struct SelfAskWithSearchAgent {
    /// Language model for reasoning
    llm: std::sync::Arc<dyn crate::core::language_models::ChatModel>,

    /// Tool definitions for passing to language model
    tool_definitions: Vec<crate::core::language_models::ToolDefinition>,

    /// System instructions
    system_prompt: String,

    /// Few-shot examples demonstrating the self-ask pattern
    examples: Vec<String>,
}

impl std::fmt::Debug for SelfAskWithSearchAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelfAskWithSearchAgent")
            .field("llm", &"<ChatModel>")
            .field("tool_definitions", &self.tool_definitions.len())
            .field(
                "system_prompt",
                &format!("[{} chars]", self.system_prompt.len()),
            )
            .field("examples", &self.examples.len())
            .finish()
    }
}

impl SelfAskWithSearchAgent {
    /// Creates a new Self-Ask with Search agent.
    ///
    /// This agent decomposes complex questions into simpler sub-questions,
    /// using a search tool to find intermediate answers before synthesizing
    /// the final response.
    ///
    /// # Panics
    ///
    /// Panics if tools doesn't contain exactly one tool named "Intermediate Answer".
    /// Use [`Self::try_new`] for a fallible version.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let agent = SelfAskWithSearchAgent::new(
    ///     model,
    ///     vec![intermediate_answer_tool],
    ///     "Answer questions by breaking them down into sub-questions.",
    /// );
    /// ```
    pub fn new(
        llm: std::sync::Arc<dyn crate::core::language_models::ChatModel>,
        tools: Vec<std::sync::Arc<dyn crate::core::tools::Tool>>,
        system_prompt: impl Into<String>,
    ) -> Self {
        Self::try_new(llm, tools, system_prompt)
            .expect("SelfAskWithSearchAgent requires exactly one tool named 'Intermediate Answer'")
    }

    /// Creates a new Self-Ask with Search agent, returning an error if configuration is invalid.
    ///
    /// # Errors
    ///
    /// Returns [`AgentConfigError::InvalidToolCount`] if not exactly one tool is provided.
    /// Returns [`AgentConfigError::InvalidToolName`] if the tool isn't named "Intermediate Answer".
    pub fn try_new(
        llm: std::sync::Arc<dyn crate::core::language_models::ChatModel>,
        tools: Vec<std::sync::Arc<dyn crate::core::tools::Tool>>,
        system_prompt: impl Into<String>,
    ) -> std::result::Result<Self, AgentConfigError> {
        if tools.len() != 1 {
            return Err(AgentConfigError::InvalidToolCount { count: tools.len() });
        }
        if tools[0].name() != "Intermediate Answer" {
            return Err(AgentConfigError::InvalidToolName {
                name: tools[0].name().to_string(),
            });
        }

        let examples = Self::default_examples();
        let tool_definitions = crate::core::tools::tools_to_definitions(&tools);
        Ok(Self {
            llm,
            tool_definitions,
            system_prompt: system_prompt.into(),
            examples,
        })
    }

    fn default_examples() -> Vec<String> {
        vec![
            r"Question: Who lived longer, Muhammad Ali or Alan Turing?
Are follow up questions needed here: Yes.
Follow up: How old was Muhammad Ali when he died?
Intermediate answer: Muhammad Ali was 74 years old when he died.
Follow up: How old was Alan Turing when he died?
Intermediate answer: Alan Turing was 41 years old when he died.
So the final answer is: Muhammad Ali"
                .to_string(),
            r"Question: When was the founder of craigslist born?
Are follow up questions needed here: Yes.
Follow up: Who was the founder of craigslist?
Intermediate answer: Craigslist was founded by Craig Newmark.
Follow up: When was Craig Newmark born?
Intermediate answer: Craig Newmark was born on December 6, 1952.
So the final answer is: December 6, 1952"
                .to_string(),
            r"Question: Who was the maternal grandfather of George Washington?
Are follow up questions needed here: Yes.
Follow up: Who was the mother of George Washington?
Intermediate answer: The mother of George Washington was Mary Ball Washington.
Follow up: Who was the father of Mary Ball Washington?
Intermediate answer: The father of Mary Ball Washington was Joseph Ball.
So the final answer is: Joseph Ball"
                .to_string(),
            r"Question: Are both the directors of Jaws and Casino Royale from the same country?
Are follow up questions needed here: Yes.
Follow up: Who is the director of Jaws?
Intermediate answer: The director of Jaws is Steven Spielberg.
Follow up: Where is Steven Spielberg from?
Intermediate answer: The United States.
Follow up: Who is the director of Casino Royale?
Intermediate answer: The director of Casino Royale is Martin Campbell.
Follow up: Where is Martin Campbell from?
Intermediate answer: New Zealand.
So the final answer is: No"
                .to_string(),
        ]
    }

    pub(super) fn format_scratchpad(&self, steps: &[AgentStep]) -> String {
        let mut scratchpad = String::new();

        for step in steps {
            // M-989: Use explicit fallback instead of unwrap_or_default() to avoid silent errors
            let action_input = match &step.action.tool_input {
                crate::core::tools::ToolInput::String(s) => s.clone(),
                crate::core::tools::ToolInput::Structured(v) => {
                    serde_json::to_string(v).unwrap_or_else(|_| "[structured input]".to_string())
                }
            };
            scratchpad.push_str(&format!("\nFollow up: {action_input}"));
            scratchpad.push_str(&format!("\nIntermediate answer: {}", step.observation));
        }

        scratchpad
    }

    pub(super) fn parse_output(&self, text: &str) -> Result<AgentDecision> {
        use crate::core::error::Error;

        let text = text.trim();
        let last_line = text.lines().last().unwrap_or(text);

        let followup_patterns = ["Follow up:", "Followup:"];
        let has_followup = followup_patterns
            .iter()
            .any(|pattern| last_line.contains(pattern));

        if has_followup {
            let after_colon = last_line
                .split(':')
                .nth(1)
                .ok_or_else(|| {
                    Error::Agent(format!("Could not parse follow-up question from: {text}"))
                })?
                .trim();

            return Ok(AgentDecision::Action(super::AgentAction {
                tool: "Intermediate Answer".to_string(),
                tool_input: crate::core::tools::ToolInput::String(after_colon.to_string()),
                log: text.to_string(),
            }));
        }

        let finish_string = "So the final answer is:";
        if last_line.contains(finish_string) {
            let answer = last_line
                .split(finish_string)
                .nth(1)
                .ok_or_else(|| {
                    Error::Agent(format!("Could not extract final answer from: {text}"))
                })?
                .trim()
                .to_string();

            return Ok(AgentDecision::Finish(AgentFinish {
                output: answer.clone(),
                log: text.to_string(),
            }));
        }

        Err(Error::Agent(format!(
            "Could not parse self-ask output. Expected 'Follow up:' or 'So the final answer is:', got: {text}"
        )))
    }
}

#[async_trait::async_trait]
impl Agent for SelfAskWithSearchAgent {
    async fn plan(&self, input: &str, intermediate_steps: &[AgentStep]) -> Result<AgentDecision> {
        use crate::core::messages::{BaseMessage, Message};

        let mut prompt_parts = vec![self.system_prompt.clone()];
        prompt_parts.extend(self.examples.clone());

        let scratchpad = self.format_scratchpad(intermediate_steps);
        prompt_parts.push(format!(
            "Question: {input}\nAre followup questions needed here:{scratchpad}"
        ));

        let full_prompt = prompt_parts.join("\n\n");
        let messages = vec![BaseMessage::from(Message::human(full_prompt))];

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
                crate::core::error::Error::Agent("No generations returned from LLM".to_string())
            })?
            .message
            .content()
            .as_text();

        self.parse_output(&text)
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

    struct MockSearchTool {
        name: String,
    }

    impl MockSearchTool {
        fn intermediate_answer() -> Self {
            Self {
                name: "Intermediate Answer".to_string(),
            }
        }

        fn with_name(name: impl Into<String>) -> Self {
            Self { name: name.into() }
        }
    }

    #[async_trait::async_trait]
    impl crate::core::tools::Tool for MockSearchTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "Search tool for factual questions"
        }
        async fn _call(
            &self,
            _input: ToolInput,
        ) -> crate::core::error::Result<String> {
            Ok("42 years old".to_string())
        }
    }

    // ===================== Mock ChatModel =====================

    struct MockChatModel {
        response: String,
    }

    impl MockChatModel {
        fn with_response(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
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
            use crate::core::messages::Message;

            Ok(crate::core::language_models::ChatResult {
                generations: vec![crate::core::language_models::ChatGeneration {
                    message: crate::core::messages::BaseMessage::from(
                        Message::ai(self.response.clone()),
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

    // ===================== Constructor Tests =====================

    #[test]
    fn new_succeeds_with_correct_tool() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());

        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "System prompt");
        assert_eq!(agent.system_prompt, "System prompt");
    }

    #[test]
    fn try_new_succeeds_with_correct_tool() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());

        let result = SelfAskWithSearchAgent::try_new(model, vec![tool], "prompt");
        assert!(result.is_ok());
    }

    #[test]
    fn try_new_fails_with_zero_tools() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let result = SelfAskWithSearchAgent::try_new(model, vec![], "prompt");

        assert!(result.is_err());
        match result.unwrap_err() {
            AgentConfigError::InvalidToolCount { count } => assert_eq!(count, 0),
            _ => panic!("Expected InvalidToolCount error"),
        }
    }

    #[test]
    fn try_new_fails_with_multiple_tools() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool1: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let tool2: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::with_name("Other Tool"));

        let result = SelfAskWithSearchAgent::try_new(model, vec![tool1, tool2], "prompt");

        assert!(result.is_err());
        match result.unwrap_err() {
            AgentConfigError::InvalidToolCount { count } => assert_eq!(count, 2),
            _ => panic!("Expected InvalidToolCount error"),
        }
    }

    #[test]
    fn try_new_fails_with_wrong_tool_name() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::with_name("Wrong Name"));

        let result = SelfAskWithSearchAgent::try_new(model, vec![tool], "prompt");

        assert!(result.is_err());
        match result.unwrap_err() {
            AgentConfigError::InvalidToolName { name } => {
                assert_eq!(name, "Wrong Name");
            }
            _ => panic!("Expected InvalidToolName error"),
        }
    }

    #[test]
    #[should_panic(expected = "exactly one tool")]
    fn new_panics_with_zero_tools() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let _ = SelfAskWithSearchAgent::new(model, vec![], "prompt");
    }

    #[test]
    #[should_panic(expected = "exactly one tool")]
    fn new_panics_with_wrong_tool_name() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::with_name("Wrong"));
        let _ = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");
    }

    // ===================== default_examples Tests =====================

    #[test]
    fn default_examples_returns_4_examples() {
        let examples = SelfAskWithSearchAgent::default_examples();
        assert_eq!(examples.len(), 4);
    }

    #[test]
    fn default_examples_contain_muhammad_ali() {
        let examples = SelfAskWithSearchAgent::default_examples();
        assert!(examples[0].contains("Muhammad Ali"));
    }

    #[test]
    fn default_examples_contain_craigslist() {
        let examples = SelfAskWithSearchAgent::default_examples();
        assert!(examples[1].contains("craigslist"));
    }

    #[test]
    fn default_examples_contain_george_washington() {
        let examples = SelfAskWithSearchAgent::default_examples();
        assert!(examples[2].contains("George Washington"));
    }

    #[test]
    fn default_examples_contain_jaws() {
        let examples = SelfAskWithSearchAgent::default_examples();
        assert!(examples[3].contains("Jaws"));
    }

    #[test]
    fn default_examples_show_self_ask_pattern() {
        let examples = SelfAskWithSearchAgent::default_examples();
        for example in &examples {
            assert!(example.contains("Question:"));
            assert!(example.contains("Follow up:"));
            assert!(example.contains("Intermediate answer:"));
            assert!(example.contains("So the final answer is:"));
        }
    }

    // ===================== format_scratchpad Tests =====================

    #[test]
    fn format_scratchpad_empty_steps() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let scratchpad = agent.format_scratchpad(&[]);
        assert!(scratchpad.is_empty());
    }

    #[test]
    fn format_scratchpad_single_step_string_input() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let steps = vec![AgentStep {
            action: super::super::AgentAction {
                tool: "Intermediate Answer".to_string(),
                tool_input: ToolInput::String("How old was X?".to_string()),
                log: "log".to_string(),
            },
            observation: "42 years old".to_string(),
        }];

        let scratchpad = agent.format_scratchpad(&steps);
        assert!(scratchpad.contains("Follow up: How old was X?"));
        assert!(scratchpad.contains("Intermediate answer: 42 years old"));
    }

    #[test]
    fn format_scratchpad_multiple_steps() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let steps = vec![
            AgentStep {
                action: super::super::AgentAction {
                    tool: "Intermediate Answer".to_string(),
                    tool_input: ToolInput::String("First question?".to_string()),
                    log: "log1".to_string(),
                },
                observation: "First answer".to_string(),
            },
            AgentStep {
                action: super::super::AgentAction {
                    tool: "Intermediate Answer".to_string(),
                    tool_input: ToolInput::String("Second question?".to_string()),
                    log: "log2".to_string(),
                },
                observation: "Second answer".to_string(),
            },
        ];

        let scratchpad = agent.format_scratchpad(&steps);
        assert!(scratchpad.contains("Follow up: First question?"));
        assert!(scratchpad.contains("Intermediate answer: First answer"));
        assert!(scratchpad.contains("Follow up: Second question?"));
        assert!(scratchpad.contains("Intermediate answer: Second answer"));
    }

    #[test]
    fn format_scratchpad_structured_input() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let steps = vec![AgentStep {
            action: super::super::AgentAction {
                tool: "Intermediate Answer".to_string(),
                tool_input: ToolInput::Structured(serde_json::json!({"query": "test"})),
                log: "log".to_string(),
            },
            observation: "result".to_string(),
        }];

        let scratchpad = agent.format_scratchpad(&steps);
        // Structured input should be JSON-serialized
        assert!(scratchpad.contains("Follow up:"));
        assert!(scratchpad.contains("query"));
    }

    // ===================== parse_output Tests =====================

    #[test]
    fn parse_output_follow_up_colon() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let output = "Yes.\nFollow up: How old was X?";
        let result = agent.parse_output(output).expect("parse");

        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "Intermediate Answer");
                match action.tool_input {
                    ToolInput::String(s) => assert_eq!(s, "How old was X?"),
                    _ => panic!("Expected String input"),
                }
            }
            _ => panic!("Expected Action"),
        }
    }

    #[test]
    fn parse_output_followup_no_space() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let output = "Yes.\nFollowup: Who founded X?";
        let result = agent.parse_output(output).expect("parse");

        match result {
            AgentDecision::Action(action) => {
                assert_eq!(action.tool, "Intermediate Answer");
            }
            _ => panic!("Expected Action"),
        }
    }

    #[test]
    fn parse_output_final_answer() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let output = "Some reasoning.\nSo the final answer is: Muhammad Ali";
        let result = agent.parse_output(output).expect("parse");

        match result {
            AgentDecision::Finish(finish) => {
                assert_eq!(finish.output, "Muhammad Ali");
                assert!(finish.log.contains("So the final answer is:"));
            }
            _ => panic!("Expected Finish"),
        }
    }

    #[test]
    fn parse_output_final_answer_multiword() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let output = "So the final answer is: December 6, 1952";
        let result = agent.parse_output(output).expect("parse");

        match result {
            AgentDecision::Finish(finish) => {
                assert_eq!(finish.output, "December 6, 1952");
            }
            _ => panic!("Expected Finish"),
        }
    }

    #[test]
    fn parse_output_no_match_returns_error() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let output = "This is just some random text without any pattern.";
        let result = agent.parse_output(output);

        assert!(result.is_err());
    }

    #[test]
    fn parse_output_trims_whitespace() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let output = "  \n  So the final answer is: Yes  \n  ";
        let result = agent.parse_output(output).expect("parse");

        match result {
            AgentDecision::Finish(finish) => {
                assert_eq!(finish.output, "Yes");
            }
            _ => panic!("Expected Finish"),
        }
    }

    #[test]
    fn parse_output_uses_last_line() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        // Multiple lines, but the last line determines the action
        let output = "Line 1\nLine 2\nSo the final answer is: Answer";
        let result = agent.parse_output(output).expect("parse");

        assert!(matches!(result, AgentDecision::Finish(_)));
    }

    #[test]
    fn parse_output_action_preserves_log() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let output = "Are followup questions needed: Yes\nFollow up: Who is X?";
        let result = agent.parse_output(output).expect("parse");

        match result {
            AgentDecision::Action(action) => {
                assert!(action.log.contains("Are followup questions needed"));
            }
            _ => panic!("Expected Action"),
        }
    }

    // ===================== input_keys / output_keys Tests =====================

    #[test]
    fn input_keys_returns_input() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        assert_eq!(agent.input_keys(), vec!["input"]);
    }

    #[test]
    fn output_keys_returns_output() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        assert_eq!(agent.output_keys(), vec!["output"]);
    }

    // ===================== Debug Impl Tests =====================

    #[test]
    fn debug_impl_shows_struct_name() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let debug = format!("{:?}", agent);
        assert!(debug.contains("SelfAskWithSearchAgent"));
    }

    #[test]
    fn debug_impl_shows_tool_count() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let debug = format!("{:?}", agent);
        // Should show tool_definitions count, which is 1
        assert!(debug.contains("1"));
    }

    #[test]
    fn debug_impl_shows_prompt_length() {
        let model = Arc::new(MockChatModel::with_response("response"));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "A short prompt");

        let debug = format!("{:?}", agent);
        // Should show "[14 chars]" for "A short prompt"
        assert!(debug.contains("chars]"));
    }

    // ===================== plan Tests =====================

    #[tokio::test]
    async fn plan_with_follow_up_response() {
        let model = Arc::new(MockChatModel::with_response(
            "Are follow up questions needed here: Yes.\nFollow up: How old was X?"
        ));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let result = agent.plan("Complex question", &[]).await.expect("plan");
        assert!(matches!(result, AgentDecision::Action(_)));
    }

    #[tokio::test]
    async fn plan_with_final_answer_response() {
        let model = Arc::new(MockChatModel::with_response(
            "So the final answer is: The Answer"
        ));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        let result = agent.plan("Simple question", &[]).await.expect("plan");
        match result {
            AgentDecision::Finish(finish) => {
                assert_eq!(finish.output, "The Answer");
            }
            _ => panic!("Expected Finish"),
        }
    }

    #[tokio::test]
    async fn plan_includes_examples() {
        // The plan method includes default examples in the prompt
        // We can't easily verify this without inspecting the messages,
        // but we can verify the agent initializes with examples
        let model = Arc::new(MockChatModel::with_response(
            "So the final answer is: Yes"
        ));
        let tool: Arc<dyn crate::core::tools::Tool> =
            Arc::new(MockSearchTool::intermediate_answer());
        let agent = SelfAskWithSearchAgent::new(model, vec![tool], "prompt");

        assert_eq!(agent.examples.len(), 4);
    }
}
